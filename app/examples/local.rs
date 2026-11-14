//! Run the shell in a real terminal: `just tui`.

use app::{Action, App, Frontend, Key};
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::crossterm::execute;

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    execute!(std::io::stdout(), SetCursorStyle::BlinkingBlock)?;
    let mut app = App::new(Frontend::Local);
    let result = loop {
        if let Err(e) = terminal.draw(|f| app.render(f)) {
            break Err(e);
        }
        let ev = match event::read() {
            Ok(ev) => ev,
            Err(e) => break Err(e),
        };
        let Event::Key(k) = ev else { continue };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let key = match k.code {
            KeyCode::Char('c') if ctrl => Key::Interrupt,
            KeyCode::Char('l') if ctrl => Key::ClearScreen,
            KeyCode::Char('d') if ctrl => Key::Eof,
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Enter => Key::Enter,
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Up => Key::Up,
            KeyCode::Down => Key::Down,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            _ => continue,
        };
        if app.handle_key(key) == Some(Action::Quit) {
            break Ok(());
        }
    };
    let _ = execute!(std::io::stdout(), SetCursorStyle::DefaultUserShape);
    ratatui::restore();
    result
}
