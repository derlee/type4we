//! Text-injection planning.
//!
//! The Windows crate turns [`Stroke`]s into `SendInput` events with
//! `KEYEVENTF_UNICODE`, and uses the clipboard plan only as a fallback.
//! Planning stays here so the encoding rules are tested without Win32.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InjectionMethod {
    /// Unicode key events, then clipboard if `SendInput` inserts nothing.
    #[default]
    Auto,
    /// `KEYEVENTF_UNICODE` only.
    Unicode,
    /// Clipboard plus Ctrl+V, then restore the previous Unicode clipboard.
    Clipboard,
}

impl InjectionMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Unicode => "unicode",
            Self::Clipboard => "clipboard",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "auto" => Some(Self::Auto),
            "unicode" => Some(Self::Unicode),
            "clipboard" => Some(Self::Clipboard),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    /// One UTF-16 code unit, sent as a unicode key down/up pair.
    Unicode(u16),
    /// A newline in the transcript becomes an Enter key, which more editors
    /// honor than a raw line-feed character.
    Enter,
}

/// `SendInput` reported that it inserted fewer events than requested.
pub fn send_input_failed(requested: u32, inserted: u32) -> bool {
    requested > 0 && inserted < requested
}

pub fn plan_strokes(text: &str) -> Vec<Stroke> {
    let mut strokes = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                strokes.push(Stroke::Enter);
            }
            '\n' => strokes.push(Stroke::Enter),
            other => {
                let mut buf = [0u16; 2];
                for unit in other.encode_utf16(&mut buf) {
                    strokes.push(Stroke::Unicode(*unit));
                }
            }
        }
    }
    strokes
}

pub fn uses_clipboard(method: InjectionMethod, requested: u32, inserted: u32) -> bool {
    match method {
        InjectionMethod::Clipboard => true,
        InjectionMethod::Unicode => false,
        InjectionMethod::Auto => send_input_failed(requested, inserted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_cjk_are_single_utf16_units() {
        let strokes = plan_strokes("A你");
        assert_eq!(
            strokes,
            vec![
                Stroke::Unicode(u16::from(b'A')),
                Stroke::Unicode('你' as u16),
            ]
        );
    }

    #[test]
    fn emoji_is_a_surrogate_pair() {
        let strokes = plan_strokes("😀");
        let mut buf = [0u16; 2];
        let units: Vec<u16> = '😀'.encode_utf16(&mut buf).to_vec();
        assert_eq!(units.len(), 2);
        assert_eq!(
            strokes,
            vec![Stroke::Unicode(units[0]), Stroke::Unicode(units[1])]
        );
    }

    #[test]
    fn newlines_become_enter_and_crlf_is_one_enter() {
        assert_eq!(
            plan_strokes("a\nb"),
            vec![
                Stroke::Unicode(u16::from(b'a')),
                Stroke::Enter,
                Stroke::Unicode(u16::from(b'b')),
            ]
        );
        assert_eq!(
            plan_strokes("a\r\nb")
                .iter()
                .filter(|s| **s == Stroke::Enter)
                .count(),
            1
        );
    }

    #[test]
    fn auto_falls_back_only_when_sendinput_inserts_nothing_useful() {
        assert!(!uses_clipboard(InjectionMethod::Auto, 4, 4));
        assert!(uses_clipboard(InjectionMethod::Auto, 4, 0));
        assert!(uses_clipboard(InjectionMethod::Auto, 4, 2));
        assert!(!uses_clipboard(InjectionMethod::Unicode, 4, 0));
        assert!(uses_clipboard(InjectionMethod::Clipboard, 4, 4));
        assert!(!send_input_failed(0, 0));
    }
}
