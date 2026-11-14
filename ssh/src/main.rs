//! `ssh klercke.com`: serves the site's shell over SSH.
//!
//! No real shell, no exec, no SSH forwarding.
//! Every visitor gets their own `app::App` rendered in their terminal.
//! Configured with environment variables:
//!
//! - `WEBSSH_LISTEN`: address to listen on (default `[::]:22`)
//! - `WEBSSH_HOST_KEY`: host key path. Defaults to `ssh_host_ed25519_key` in
//!   systemd's `$STATE_DIRECTORY`, or in the working directory. A missing key
//!   is generated on first start.

mod input;
mod limits;
mod session;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{env, net::IpAddr};

use russh::keys::ssh_key::LineEnding;
use russh::keys::{Algorithm, PrivateKey};
use russh::server::Config;
use russh::{Disconnect, MethodKind, MethodSet, SshId};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal::unix::{SignalKind, signal};

use crate::limits::{Limits, Permit};
use crate::session::{Connection, Error};

const MAX_CONNECTIONS: usize = 64;
const MAX_PER_CLIENT: u32 = 3;
const MAX_PER_MINUTE: u32 = 10;
const IDLE_TIMEOUT: Duration = Duration::from_mins(10);
const MAX_SESSION: Duration = Duration::from_hours(1);

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("webssh: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Error> {
    let listen = env::var("WEBSSH_LISTEN").unwrap_or_else(|_| "[::]:22".into());
    let key = load_or_create_key(&host_key_path())?;

    let config = Arc::new(Config {
        server_id: SshId::Standard(Cow::Borrowed("SSH-2.0-webssh")),
        methods: MethodSet::from(&[MethodKind::None][..]),
        keys: vec![key],
        auth_rejection_time_initial: Some(Duration::ZERO),
        max_auth_attempts: 3,
        inactivity_timeout: Some(IDLE_TIMEOUT),
        keepalive_interval: Some(Duration::from_secs(30)),
        nodelay: true,
        ..Config::default()
    });
    let limits = Limits::new(MAX_CONNECTIONS, MAX_PER_CLIENT, MAX_PER_MINUTE);

    let listener = TcpListener::bind(&listen).await?;
    eprintln!(
        "webssh {} listening on {}",
        env!("CARGO_PKG_VERSION"),
        listener.local_addr()?
    );

    let mut sigterm = signal(SignalKind::terminate())?;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, addr) = match accepted {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("accept failed: {e}");
                        continue;
                    }
                };
                let ip = addr.ip().to_canonical();
                match limits.admit(ip) {
                    Ok(permit) => {
                        tokio::spawn(serve(Arc::clone(&config), stream, ip, permit));
                    }
                    Err(why) => eprintln!("{ip}: rejected, {why}"),
                }
            }
            _ = sigterm.recv() => break,
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    eprintln!("webssh shutting down");
    Ok(())
}

async fn serve(config: Arc<Config>, stream: TcpStream, ip: IpAddr, permit: Permit) {
    let started = Instant::now();
    eprintln!("{ip}: connected");
    match russh::server::run_stream(config, stream, Connection::new(ip, permit)).await {
        Ok(running) => {
            let handle = running.handle();
            match tokio::time::timeout(MAX_SESSION, running).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("{ip}: session ended with error: {e}"),
                Err(_) => {
                    let _ = handle
                        .disconnect(
                            Disconnect::ByApplication,
                            "Session time limit reached. Come back any time!".into(),
                            "en".into(),
                        )
                        .await;
                }
            }
        }
        Err(e) => eprintln!("{ip}: handshake failed: {e}"),
    }
    eprintln!("{ip}: disconnected after {}s", started.elapsed().as_secs());
}

fn host_key_path() -> PathBuf {
    if let Some(path) = env::var_os("WEBSSH_HOST_KEY") {
        return path.into();
    }
    let dir = env::var("STATE_DIRECTORY")
        .ok()
        .and_then(|dirs| dirs.split(':').next().map(PathBuf::from))
        .unwrap_or_default();
    dir.join("ssh_host_ed25519_key")
}

fn load_or_create_key(path: &Path) -> Result<PrivateKey, Error> {
    if path.exists() {
        return Ok(russh::keys::load_secret_key(path, None)?);
    }
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519)?;
    key.write_openssh_file(path, LineEnding::LF)?;
    eprintln!("generated a new host key at {}", path.display());
    Ok(key)
}
