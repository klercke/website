//! Connection limits.
//! Checked before the SSH handshake starts, so floods have less impact.

use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const RATE_WINDOW: Duration = Duration::from_mins(1);
/// Clean up idle entries once the table grows past this.
const TABLE_SOFT_MAX: usize = 4096;

/// IPv4 addresses are limited individually, IPv6 by /64, since one customer
/// usually gets a whole /64.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Client {
    V4([u8; 4]),
    V6([u8; 8]),
}

impl From<IpAddr> for Client {
    fn from(ip: IpAddr) -> Self {
        match ip.to_canonical() {
            IpAddr::V4(v4) => Self::V4(v4.octets()),
            IpAddr::V6(v6) => {
                let mut prefix = [0; 8];
                prefix.copy_from_slice(&v6.octets()[..8]);
                Self::V6(prefix)
            }
        }
    }
}

struct ClientState {
    active: u32,
    window_start: Instant,
    recent: u32,
}

pub struct Limits {
    total: Arc<Semaphore>,
    clients: Mutex<HashMap<Client, ClientState>>,
    max_per_client: u32,
    max_per_minute: u32,
}

#[derive(Debug)]
pub enum Rejected {
    Full,
    TooManyConnections,
    TooFast,
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Full => "server full",
            Self::TooManyConnections => "too many connections from this address",
            Self::TooFast => "connecting too often",
        })
    }
}

/// Held for the life of a connection; releases its slots when dropped.
pub struct Permit {
    _total: OwnedSemaphorePermit,
    client: Client,
    limits: Arc<Limits>,
}

impl Limits {
    pub fn new(max_total: usize, max_per_client: u32, max_per_minute: u32) -> Arc<Self> {
        Arc::new(Self {
            total: Arc::new(Semaphore::new(max_total)),
            clients: Mutex::new(HashMap::new()),
            max_per_client,
            max_per_minute,
        })
    }

    pub fn admit(self: &Arc<Self>, ip: IpAddr) -> Result<Permit, Rejected> {
        let client = Client::from(ip);
        let now = Instant::now();
        let mut clients = self.clients.lock().unwrap_or_else(PoisonError::into_inner);

        if clients.len() > TABLE_SOFT_MAX {
            clients.retain(|_, s| s.active > 0 || now.duration_since(s.window_start) < RATE_WINDOW);
        }

        let state = clients.entry(client).or_insert(ClientState {
            active: 0,
            window_start: now,
            recent: 0,
        });
        if now.duration_since(state.window_start) >= RATE_WINDOW {
            state.window_start = now;
            state.recent = 0;
        }
        if state.recent >= self.max_per_minute {
            return Err(Rejected::TooFast);
        }
        state.recent += 1;
        if state.active >= self.max_per_client {
            return Err(Rejected::TooManyConnections);
        }
        let total = self
            .total
            .clone()
            .try_acquire_owned()
            .map_err(|_| Rejected::Full)?;
        state.active += 1;

        Ok(Permit {
            _total: total,
            client,
            limits: Arc::clone(self),
        })
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut clients = self
            .limits
            .clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(state) = clients.get_mut(&self.client) {
            state.active = state.active.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn per_client_limit_and_release() {
        let limits = Limits::new(10, 2, 100);
        let a = limits.admit(ip("192.0.2.1"));
        let b = limits.admit(ip("192.0.2.1"));
        assert!(a.is_ok() && b.is_ok());
        assert!(matches!(
            limits.admit(ip("192.0.2.1")),
            Err(Rejected::TooManyConnections)
        ));
        assert!(limits.admit(ip("192.0.2.2")).is_ok());
        drop(a);
        assert!(limits.admit(ip("192.0.2.1")).is_ok());
        drop(b);
    }

    #[test]
    fn ipv6_is_grouped_by_64_and_mapped_v4_is_v4() {
        let limits = Limits::new(10, 1, 100);
        let _a = limits.admit(ip("2001:db8::1"));
        assert!(matches!(
            limits.admit(ip("2001:db8::2")),
            Err(Rejected::TooManyConnections)
        ));
        let _b = limits.admit(ip("::ffff:192.0.2.9"));
        assert!(matches!(
            limits.admit(ip("192.0.2.9")),
            Err(Rejected::TooManyConnections)
        ));
    }

    #[test]
    fn rate_and_total_limits() {
        let limits = Limits::new(1, 10, 2);
        let held = limits.admit(ip("192.0.2.1"));
        assert!(held.is_ok());
        assert!(matches!(limits.admit(ip("192.0.2.2")), Err(Rejected::Full)));
        drop(held);
        assert!(limits.admit(ip("192.0.2.1")).is_ok());
        assert!(matches!(
            limits.admit(ip("192.0.2.1")),
            Err(Rejected::TooFast)
        ));
    }
}
