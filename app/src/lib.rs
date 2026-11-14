//! The shell for klercke.com
//! This UI is shared between the browser shell and SSH.
//! Each frontend translates its inputs into [`Key`], calls
//! [`App::handle_key`], acts on [`Action`]s and calls [`App::render`] to draw.

pub mod content;

use ratatui::{
    Frame,
    layout::Position,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

pub const HOST: &str = "klercke.com";
pub const USER: &str = "guest";
pub const SOURCE: &str = "https://github.com/klercke/website";

/// Longest input line accepted, in chars.
const MAX_INPUT: usize = 256;

/// Scrollback lines keps.
/// Anything older is dropped.
const MAX_SCROLLBACK: usize = 2000;

/// The drawn cursor at the end of the input line.
const CURSOR: &str = "█";

const YELLOW: Color = Color::Rgb(0xfa, 0xbd, 0x2f);
const ORANGE: Color = Color::Rgb(0xfe, 0x80, 0x19);
const AQUA: Color = Color::Rgb(0x8e, 0xc0, 0x7c);
const RED: Color = Color::Rgb(0xfb, 0x49, 0x34);
const FG: Color = Color::Rgb(0xeb, 0xdb, 0xb2);
const BG: Color = Color::Rgb(0x28, 0x28, 0x28);

/// Which frontend is driving the app.
/// Changes what `open` and `exit` do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frontend {
    Web,
    Ssh,
    Local,
}

/// Frontend-neutral key input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Up,
    Down,
    PageUp,
    PageDown,
    /// Ctrl-C: drop the current input line.
    Interrupt,
    /// Ctrl-L: clear the screen.
    ClearScreen,
    /// Ctrl-D: exit.
    Eof,
}

/// Something the frontend has to do on the app's behalf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Browser: go to this path on the main site.
    Navigate(String),
    /// SSH: disconnect. Local: quit.
    Quit,
}

pub struct App {
    frontend: Frontend,
    input: String,
    scrollback: Vec<Line<'static>>,
    history: Vec<String>,
    history_pos: Option<usize>,
    /// Lines scrolled up from the bottom.
    scroll: u16,
    cursor_visible: bool,
}

impl App {
    #[must_use]
    pub fn new(frontend: Frontend) -> Self {
        let mut app = Self {
            frontend,
            input: String::new(),
            scrollback: Vec::new(),
            history: Vec::new(),
            history_pos: None,
            scroll: 0,
            cursor_visible: true,
        };
        app.print(Line::styled(
            format!("Welcome to {HOST}. Type `help` to get started."),
            Style::new().fg(YELLOW),
        ));
        app.print(Line::styled(
            "This shell is free software (AGPL-3.0). Type `source` for the code.",
            Style::new().fg(FG).add_modifier(Modifier::DIM),
        ));
        app
    }

    pub fn handle_key(&mut self, key: Key) -> Option<Action> {
        match key {
            Key::Char(c) if !c.is_control() && self.input.chars().count() < MAX_INPUT => {
                self.input.push(c);
            }
            Key::Char(_) => {}
            Key::Backspace => {
                self.input.pop();
            }
            Key::Enter => {
                self.scroll = 0;
                let line = std::mem::take(&mut self.input);
                self.print(prompt_line(&line));
                if !line.trim().is_empty() {
                    self.history.push(line.clone());
                }
                self.history_pos = None;
                return self.run(line.trim());
            }
            Key::Up => self.history_step(true),
            Key::Down => self.history_step(false),
            Key::PageUp => self.scroll = self.scroll.saturating_add(10),
            Key::PageDown => self.scroll = self.scroll.saturating_sub(10),
            Key::Interrupt => {
                let line = std::mem::take(&mut self.input);
                self.print(prompt_line(&format!("{line}^C")));
            }
            Key::ClearScreen => self.scrollback.clear(),
            Key::Eof => return self.run("exit"),
        }
        None
    }

    /// For frontends that blink the cursor themselves (the browser).
    /// Terminal frontends leave it on and let the terminal blink it.
    pub const fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }

    pub fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let mut lines = self.scrollback.clone();
        let mut prompt = prompt_line(&self.input);
        prompt.push_span(Span::styled(
            if self.cursor_visible { CURSOR } else { " " },
            Style::new().fg(FG),
        ));
        lines.push(prompt);

        let paragraph = Paragraph::new(lines)
            .style(Style::new().fg(FG).bg(BG))
            .wrap(Wrap { trim: false });
        let total = u16::try_from(paragraph.line_count(area.width)).unwrap_or(u16::MAX);
        let bottom = total.saturating_sub(area.height);
        let offset = bottom.saturating_sub(self.scroll);
        frame.render_widget(paragraph.scroll((offset, 0)), area);

        // Terminals blink their own cursor, but many won't blink text.
        // So in a terminal, swap the drawn block for the real cursor.
        // The prompt is the last line, so the first block found from the
        // bottom is ours; if it's scrolled out of view, no position is set and
        // the cursor stays hidden.
        if self.frontend != Frontend::Web {
            let buf = frame.buffer_mut();
            let found = (area.top()..area.bottom())
                .rev()
                .flat_map(|y| {
                    (area.left()..area.right())
                        .rev()
                        .map(move |x| Position::new(x, y))
                })
                .find(|&p| buf[p].symbol() == CURSOR);
            if let Some(pos) = found {
                buf[pos].set_symbol(" ");
                frame.set_cursor_position(pos);
            }
        }
    }

    fn run(&mut self, line: &str) -> Option<Action> {
        let mut words = line.split_whitespace();
        let cmd = words.next()?;
        let arg = words.next();

        match (cmd, arg) {
            ("help", _) => self.help(),
            ("ls", _) => {
                let names: Vec<_> = content::FILES.iter().map(|f| f.name).collect();
                self.print(Line::styled(names.join("  "), Style::new().fg(AQUA)));
            }
            ("cat", Some(name)) => match content::file(name) {
                Some(f) => self.print_text(content::strip_front_matter(f.body)),
                None => self.error(format!("cat: {name}: No such file")),
            },
            ("cat", None) => self.error("usage: cat <file>".into()),
            ("clear", _) => self.scrollback.clear(),
            ("whoami", _) => self.print_text(USER),
            ("source", _) => self.print_text(&format!("This shell's source code: {SOURCE}")),
            ("open", Some(page)) => {
                let Some((_, path)) = content::PAGES.iter().find(|(n, _)| *n == page) else {
                    let names: Vec<_> = content::PAGES.iter().map(|(n, _)| *n).collect();
                    self.error(format!("open: unknown page. Try: {}", names.join(", ")));
                    return None;
                };
                if self.frontend == Frontend::Web {
                    return Some(Action::Navigate((*path).into()));
                }
                self.print_text(&format!("Open {}{path} in your browser.", content::SITE));
            }
            ("open", None) => self.error("usage: open <page>".into()),
            ("exit", _) => {
                return Some(match self.frontend {
                    Frontend::Web => Action::Navigate("/".into()),
                    Frontend::Ssh | Frontend::Local => Action::Quit,
                });
            }
            _ => self.error(format!("{cmd}: command not found. Type `help`.")),
        }
        None
    }

    fn help(&mut self) {
        let exit = match self.frontend {
            Frontend::Web => "back to the main site",
            Frontend::Ssh => "disconnect",
            Frontend::Local => "quit",
        };
        let rows = [
            ("help", "show this help"),
            ("ls", "list files"),
            ("cat <file>", "print a file"),
            ("open <page>", "open a page of the website"),
            ("whoami", "who you are"),
            ("source", "show where to get this shell's source code"),
            ("clear", "clear the screen"),
            ("exit", exit),
        ];
        for (cmd, what) in rows {
            self.print(Line::from(vec![
                Span::styled(format!("  {cmd:<14}"), Style::new().fg(YELLOW)),
                Span::raw(what),
            ]));
        }
    }

    fn history_step(&mut self, back: bool) {
        if self.history.is_empty() {
            return;
        }
        let last = self.history.len() - 1;
        self.history_pos = match (self.history_pos, back) {
            (None, true) => Some(last),
            (Some(i), true) => Some(i.saturating_sub(1)),
            (Some(i), false) if i < last => Some(i + 1),
            (_, false) => None,
        };
        self.input = self.history_pos.map_or_default(|i| self.history[i].clone());
    }

    fn print(&mut self, line: Line<'static>) {
        if self.scrollback.len() >= MAX_SCROLLBACK {
            self.scrollback
                .drain(..=self.scrollback.len() - MAX_SCROLLBACK);
        }
        self.scrollback.push(line);
    }

    fn print_text(&mut self, text: &str) {
        for l in text.lines() {
            self.print(Line::raw(l.to_owned()));
        }
    }

    fn error(&mut self, msg: String) {
        self.print(Line::styled(msg, Style::new().fg(RED)));
    }
}

fn prompt_line(input: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(USER, Style::new().fg(AQUA).add_modifier(Modifier::BOLD)),
        Span::styled("@", Style::new().fg(FG)),
        Span::styled(HOST, Style::new().fg(YELLOW).add_modifier(Modifier::BOLD)),
        Span::styled(":~$ ", Style::new().fg(ORANGE)),
        Span::raw(input.to_owned()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_line(app: &mut App, s: &str) -> Option<Action> {
        s.chars().for_each(|c| {
            app.handle_key(Key::Char(c));
        });
        app.handle_key(Key::Enter)
    }

    #[test]
    fn exit_depends_on_frontend() {
        assert_eq!(
            type_line(&mut App::new(Frontend::Web), "exit"),
            Some(Action::Navigate("/".into()))
        );
        assert_eq!(
            type_line(&mut App::new(Frontend::Ssh), "exit"),
            Some(Action::Quit)
        );
    }

    #[test]
    fn open_navigates_only_on_web() {
        assert_eq!(
            type_line(&mut App::new(Frontend::Web), "open projects"),
            Some(Action::Navigate("/projects/".into()))
        );
        assert_eq!(
            type_line(&mut App::new(Frontend::Ssh), "open projects"),
            None
        );
    }

    #[test]
    fn history_recalls_last_command() {
        let mut app = App::new(Frontend::Local);
        type_line(&mut app, "ls");
        app.handle_key(Key::Up);
        assert_eq!(app.input, "ls");
    }

    #[test]
    fn input_and_scrollback_are_capped() {
        let mut app = App::new(Frontend::Ssh);
        for _ in 0..MAX_INPUT + 10 {
            app.handle_key(Key::Char('a'));
        }
        assert_eq!(app.input.chars().count(), MAX_INPUT);
        for _ in 0..MAX_SCROLLBACK {
            type_line(&mut app, "x");
        }
        assert!(app.scrollback.len() <= MAX_SCROLLBACK);
    }
}
