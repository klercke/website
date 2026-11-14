use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};

use app::{Action, App, Frontend, Key};
use ratzilla::{
    FontAtlasConfig, SelectionMode, WebGl2Backend, WebRenderer,
    backend::webgl2::WebGl2BackendOptions,
    event::{KeyCode, KeyEvent},
    ratatui::{Terminal, style::Color},
    web_sys::{
        self, KeyboardEvent,
        wasm_bindgen::{JsCast, closure::Closure},
    },
};

/// Half a blink cycle, in milliseconds, roughly what terminals use.
const BLINK_MS: f64 = 530.0;

/// Same stack as `$mono` in the site's Sass.
const MONO: &[&str] = &[
    "ui-monospace",
    "SF Mono",
    "Cascadia Mono",
    "JetBrains Mono",
    "Menlo",
    "Consolas",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "monospace",
];

fn main() {
    console_error_panic_hook::set_once();
    if let Err(e) = run() {
        web_sys::console::error_1(&format!("shell failed to start: {e}").into());
    }
}

fn run() -> io::Result<()> {
    let app = Rc::new(RefCell::new(App::new(Frontend::Web)));

    let options = WebGl2BackendOptions::new()
        .grid_id("shell")
        .canvas_padding_color(Color::Rgb(0x28, 0x28, 0x28))
        .font_atlas_config(FontAtlasConfig::dynamic(MONO, 16.0))
        .enable_mouse_selection_with_mode(SelectionMode::Linear)
        .disable_auto_css_resize();
    let terminal = Terminal::new(WebGl2Backend::new_with_options(options)?)?;

    let window = web_sys::window().ok_or_else(|| io::Error::other("no window"))?;
    let now = || {
        web_sys::window()
            .and_then(|w| w.performance())
            .map_or(0.0, |p| p.now())
    };
    let last_key = Rc::new(Cell::new(now()));
    let on_key = Closure::<dyn FnMut(KeyboardEvent)>::new({
        let app = app.clone();
        let last_key = last_key.clone();
        move |ev: KeyboardEvent| {
            if ev.meta_key() {
                return;
            }
            let Some(key) = to_key(&KeyEvent::from(ev.clone())) else {
                return;
            };
            ev.prevent_default();
            last_key.set(now());
            if let Some(Action::Navigate(path)) = app.borrow_mut().handle_key(key)
                && let Some(window) = web_sys::window()
            {
                let _ = window.location().set_href(&path);
            }
        }
    });
    window
        .add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref())
        .map_err(|_| io::Error::other("could not listen for keys"))?;
    on_key.forget();

    terminal.draw_web(move |f| {
        let since_key = now() - last_key.get();
        app.borrow_mut()
            .set_cursor_visible((since_key / BLINK_MS).floor() % 2.0 < 1.0);
        app.borrow().render(f);
    });
    Ok(())
}

const fn to_key(ev: &KeyEvent) -> Option<Key> {
    if ev.alt {
        return None;
    }
    if ev.ctrl {
        return match ev.code {
            KeyCode::Char('c') => Some(Key::Interrupt),
            KeyCode::Char('l') => Some(Key::ClearScreen),
            KeyCode::Char('d') => Some(Key::Eof),
            _ => None,
        };
    }
    Some(match ev.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        _ => return None,
    })
}
