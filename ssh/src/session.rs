//! One SSH connection; a visitor's own `app::App` drawn into their terminal.

use std::net::IpAddr;

use crate::backend::SshBackend;
use app::{Action, App, Frontend};
use ratatui::layout::Rect;
use ratatui::{Terminal, TerminalOptions, Viewport};
use russh::server::{Auth, ChannelOpenHandle, Handler, Msg, Session};
use russh::{Channel, ChannelId, ChannelOpenFailure, Pty};

use crate::limits::Permit;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
type Term = Terminal<SshBackend>;

/// Bigger windows are clamped to this so a client can't make us render huge frames.
const MAX_COLS: u16 = 400;
const MAX_ROWS: u16 = 200;

/// Alternate screen, cleared, with a blinking block cursor (DECSCUSR 1).
const ENTER_ALT_SCREEN: &[u8] = b"\x1b[?1049h\x1b[2J\x1b[1 q";

/// Back to the visitor's own cursor shape, visible, on the main screen.
const LEAVE_ALT_SCREEN: &[u8] = b"\x1b[0 q\x1b[?25h\x1b[?1049l";

const NEEDS_PTY: &str = "klercke.com is an interactive shell. Connect with: ssh -t klercke.com\r\n";

pub struct Connection {
    peer: IpAddr,
    _permit: Permit,
    channel: Option<ChannelId>,
    size: Option<Rect>,
    terminal: Option<Term>,
    app: App,
    input: crate::input::Input,
}

impl Connection {
    pub fn new(peer: IpAddr, permit: Permit) -> Self {
        Self {
            peer,
            _permit: permit,
            channel: None,
            size: None,
            terminal: None,
            app: App::new(Frontend::Ssh),
            input: crate::input::Input::default(),
        }
    }

    fn ours(&self, channel: ChannelId) -> bool {
        self.channel == Some(channel)
    }

    fn start(
        &mut self,
        channel: ChannelId,
        area: Rect,
        session: &mut Session,
    ) -> Result<(), Error> {
        let terminal = Terminal::with_options(
            SshBackend::new(ENTER_ALT_SCREEN, area.as_size()),
            TerminalOptions {
                viewport: Viewport::Fixed(area),
            },
        )?;
        self.terminal = Some(terminal);
        self.redraw(channel, session)
    }

    fn redraw(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Error> {
        let Some(terminal) = self.terminal.as_mut() else {
            return Ok(());
        };
        let app = &self.app;
        terminal.draw(|frame| app.render(frame))?;
        let frame = terminal.backend_mut().take_output();
        if !frame.is_empty() {
            session.data(channel, frame)?;
        }
        Ok(())
    }

    fn quit(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Error> {
        let mut bye = self
            .terminal
            .take()
            .map_or_else(Vec::new, |mut t| t.backend_mut().take_output());
        bye.extend_from_slice(LEAVE_ALT_SCREEN);
        bye.extend_from_slice(b"Thanks for visiting klercke.com!\r\n");
        session.data(channel, bye)?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

fn area(cols: u32, rows: u32) -> Rect {
    let clamp = |v: u32, max: u16| u16::try_from(v).unwrap_or(max).clamp(1, max);
    Rect::new(0, 0, clamp(cols, MAX_COLS), clamp(rows, MAX_ROWS))
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "russh's Handler is async by signature; this handler is synchronous by design"
)]
impl Handler for Connection {
    type Error = Error;

    async fn auth_none(&mut self, _user: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.channel.is_some() {
            reply
                .reject(ChannelOpenFailure::AdministrativelyProhibited)
                .await;
        } else {
            self.channel = Some(channel.id());
            reply.accept().await;
        }
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        cols: u32,
        rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.ours(channel) {
            self.size = Some(area(cols, rows));
            session.channel_success(channel)?;
        } else {
            session.channel_failure(channel)?;
        }
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if !self.ours(channel) || self.terminal.is_some() {
            session.channel_failure(channel)?;
            return Ok(());
        }
        session.channel_success(channel)?;
        if let Some(size) = self.size {
            return self.start(channel, size, session);
        }
        session.data(channel, NEEDS_PTY)?;
        session.exit_status_request(channel, 1)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        cols: u32,
        rows: u32,
        _pix_width: u32,
        _pix_height: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if !self.ours(channel) {
            return Ok(());
        }
        let size = area(cols, rows);
        self.size = Some(size);
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.backend_mut().set_size(size.as_size());
            terminal.resize(size)?;
            self.redraw(channel, session)?;
        }
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if !self.ours(channel) || self.terminal.is_none() {
            return Ok(());
        }
        for key in self.input.feed(data) {
            if self.app.handle_key(key) == Some(Action::Quit) {
                return self.quit(channel, session);
            }
        }
        self.redraw(channel, session)
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        _data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        eprintln!("{}: refused exec request", self.peer);
        session.channel_failure(channel)?;
        Ok(())
    }

    async fn subsystem_request(
        &mut self,
        channel: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        eprintln!("{}: refused subsystem {name:?}", self.peer);
        session.channel_failure(channel)?;
        Ok(())
    }
}
