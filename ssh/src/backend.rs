//! A Ratatui backend for one SSH channel.
//!
//! Output is crossterm's ANSI sequences collected in a buffer that the session
//! sends to the client.
//! Everything about the terminal itself (its size, the cursor position) is
//! answered from what the SSH client reported, not by asking the server's
//! own terminal since there isn't one under systemd, and crossterm's fallback
//! (running `tput`) fails inside the sandbox.

use std::io;

use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};

pub struct SshBackend {
    inner: CrosstermBackend<Vec<u8>>,
    size: Size,
    cursor: Position,
}

impl SshBackend {
    /// `prelude` is sent before the first frame.
    pub fn new(prelude: &[u8], size: Size) -> Self {
        Self {
            inner: CrosstermBackend::new(prelude.to_vec()),
            size,
            cursor: Position::ORIGIN,
        }
    }

    /// Record a new client window size from an SSH window-change request.
    pub const fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    /// Everything written since the last call ready to send to the client.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(self.inner.writer_mut())
    }
}

impl Backend for SshBackend {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(self.cursor)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.cursor = position.into();
        self.inner.set_cursor_position(self.cursor)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::default(),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
