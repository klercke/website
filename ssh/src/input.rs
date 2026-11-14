//! Turns the raw bytes a terminal sends over SSH into [`app::Key`]s.

use app::Key;

/// Longest escape sequence we wait for before giving up on it.
const MAX_ESCAPE: usize = 16;

/// Stateful decoder:
/// Keeps a partial escape sequence or UTF-8 character when one is plit across
/// two SSH packets.
#[derive(Default)]
pub struct Input {
    pending: Vec<u8>,
}

enum Parsed {
    /// A complete token of this many bytes, which may or may not be a key we use.
    Done(Option<Key>, usize),
    /// The rest of the buffer is the start of a token; wait for more bytes.
    Incomplete,
}

impl Input {
    pub fn feed(&mut self, data: &[u8]) -> Vec<Key> {
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(data);

        let mut keys = Vec::new();
        let mut rest = buf.as_slice();
        while !rest.is_empty() {
            match parse(rest) {
                Parsed::Done(key, len) => {
                    keys.extend(key);
                    rest = &rest[len..];
                }
                Parsed::Incomplete => {
                    self.pending = rest.to_vec();
                    break;
                }
            }
        }
        keys
    }
}

fn parse(b: &[u8]) -> Parsed {
    match b[0] {
        b'\r' => Parsed::Done(
            Some(Key::Enter),
            if b.get(1) == Some(&b'\n') { 2 } else { 1 },
        ),
        b'\n' => Parsed::Done(Some(Key::Enter), 1),
        0x7f | 0x08 => Parsed::Done(Some(Key::Backspace), 1),
        0x03 => Parsed::Done(Some(Key::Interrupt), 1),
        0x04 => Parsed::Done(Some(Key::Eof), 1),
        0x0c => Parsed::Done(Some(Key::ClearScreen), 1),
        0x1b => escape(b),
        0x20..=0x7e => Parsed::Done(Some(Key::Char(char::from(b[0]))), 1),
        0xc0..=0xf7 => utf8(b),
        _ => Parsed::Done(None, 1),
    }
}

/// ESC sequences.
/// Only arrows and page up/down mean anything to the shell.
fn escape(b: &[u8]) -> Parsed {
    match b.get(1) {
        // A lone ESC is the Escape key; the shell doesn't use it.
        None => Parsed::Done(None, 1),
        Some(b'[' | b'O') => {
            let Some(end) = b[2..].iter().position(|c| (0x40..=0x7e).contains(c)) else {
                return if b.len() >= MAX_ESCAPE {
                    Parsed::Done(None, b.len())
                } else {
                    Parsed::Incomplete
                };
            };
            let key = match &b[2..=end + 2] {
                b"A" => Some(Key::Up),
                b"B" => Some(Key::Down),
                b"5~" => Some(Key::PageUp),
                b"6~" => Some(Key::PageDown),
                _ => None,
            };
            Parsed::Done(key, end + 3)
        }
        // Alt+key arrives as ESC followed by the key.
        Some(_) => Parsed::Done(None, 2),
    }
}

fn utf8(b: &[u8]) -> Parsed {
    let len = match b[0] {
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    };
    if b.len() < len {
        return Parsed::Incomplete;
    }
    std::str::from_utf8(&b[..len]).map_or(Parsed::Done(None, 1), |s| {
        Parsed::Done(s.chars().next().map(Key::Char), len)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(chunks: &[&[u8]]) -> Vec<Key> {
        let mut input = Input::default();
        chunks.iter().flat_map(|c| input.feed(c)).collect()
    }

    #[test]
    fn typing_and_control_keys() {
        assert_eq!(
            keys(&[b"ls\r"]),
            [Key::Char('l'), Key::Char('s'), Key::Enter]
        );
        assert_eq!(
            keys(&[b"\x7f\x03\x04\x0c"]),
            [Key::Backspace, Key::Interrupt, Key::Eof, Key::ClearScreen]
        );
        assert_eq!(
            keys(&[b"a\r\nb"]),
            [Key::Char('a'), Key::Enter, Key::Char('b')]
        );
    }

    #[test]
    fn arrows_and_paging() {
        assert_eq!(
            keys(&[b"\x1b[A\x1b[B\x1bOA\x1b[5~\x1b[6~"]),
            [Key::Up, Key::Down, Key::Up, Key::PageUp, Key::PageDown]
        );
    }

    #[test]
    fn unknown_sequences_are_dropped() {
        assert_eq!(keys(&[b"\x1b[C\x1b[1;5Dx"]), [Key::Char('x')]);
        assert_eq!(keys(&[b"\x1bx"]), []);
    }

    #[test]
    fn split_across_packets() {
        assert_eq!(keys(&[b"\x1b[", b"A"]), [Key::Up]);
        assert_eq!(
            keys(&["é".as_bytes()[..1].as_ref(), &"é".as_bytes()[1..]]),
            [Key::Char('é')]
        );
    }

    #[test]
    fn runaway_escape_is_discarded() {
        let junk = [b"\x1b[".as_slice(), &[b'1'; 40]].concat();
        assert_eq!(keys(&[&junk, b"x"]), [Key::Char('x')]);
    }
}
