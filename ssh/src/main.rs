//! Allows the shell version of the site to be accessed at `ssh klercke.com`.
//!
//! Not implemented yet. Plan:
//! - russh server, `none` auth accepted, no exec/subsystem/forwarding
//! - one `app::App` per session, Ratatui backend writing to the channel
//! - pty-req/window-change set the viewport size
//! - connection limit, per-IP rate limit, idle timeout
//! - host key path from argv/env so systemd can point it at a `StateDirectory`

fn main() {
    eprintln!("webssh {}: not implemented yet", env!("CARGO_PKG_VERSION"));
    let _ = app::App::new(app::Frontend::Ssh);
    std::process::exit(1);
}
