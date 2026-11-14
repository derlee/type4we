//! Hotkey chord matching for a low-level keyboard hook.
//!
//! The hook itself is Windows-specific. This module decides when a chord
//! becomes active, which edge that produces, and which key events to swallow
//! so the chord does not land in the focused app (Right Alt opening menus,
//! the Windows key opening Start).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Activation {
    PushToTalk,
    Toggle,
}

impl Activation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PushToTalk => "push_to_talk",
            Self::Toggle => "toggle",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "push_to_talk" | "ptt" | "hold" => Some(Self::PushToTalk),
            "toggle" => Some(Self::Toggle),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NamedKey {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Space,
    Tab,
    Enter,
    Escape,
    Minus,
    Equal,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Pause,
    Scroll,
}

impl NamedKey {
    fn parse(token: &str) -> Option<Self> {
        if token.len() == 1 {
            let ch = token.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                return letter(ch.to_ascii_uppercase());
            }
            if ch.is_ascii_digit() {
                return digit(ch);
            }
        }
        match token {
            "space" => Some(Self::Space),
            "tab" => Some(Self::Tab),
            "enter" | "return" => Some(Self::Enter),
            "esc" | "escape" => Some(Self::Escape),
            "minus" | "-" => Some(Self::Minus),
            "equal" | "=" => Some(Self::Equal),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "home" => Some(Self::Home),
            "end" => Some(Self::End),
            "pageup" => Some(Self::PageUp),
            "pagedown" => Some(Self::PageDown),
            "insert" => Some(Self::Insert),
            "delete" | "del" => Some(Self::Delete),
            "pause" => Some(Self::Pause),
            "scroll" | "scrolllock" => Some(Self::Scroll),
            "f1" => Some(Self::F1),
            "f2" => Some(Self::F2),
            "f3" => Some(Self::F3),
            "f4" => Some(Self::F4),
            "f5" => Some(Self::F5),
            "f6" => Some(Self::F6),
            "f7" => Some(Self::F7),
            "f8" => Some(Self::F8),
            "f9" => Some(Self::F9),
            "f10" => Some(Self::F10),
            "f11" => Some(Self::F11),
            "f12" => Some(Self::F12),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Digit0 => "0",
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::Digit6 => "6",
            Self::Digit7 => "7",
            Self::Digit8 => "8",
            Self::Digit9 => "9",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::Space => "Space",
            Self::Tab => "Tab",
            Self::Enter => "Enter",
            Self::Escape => "Esc",
            Self::Minus => "-",
            Self::Equal => "=",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
            Self::Insert => "Insert",
            Self::Delete => "Delete",
            Self::Pause => "Pause",
            Self::Scroll => "ScrollLock",
        }
    }
}

fn letter(ch: char) -> Option<NamedKey> {
    Some(match ch {
        'A' => NamedKey::A,
        'B' => NamedKey::B,
        'C' => NamedKey::C,
        'D' => NamedKey::D,
        'E' => NamedKey::E,
        'F' => NamedKey::F,
        'G' => NamedKey::G,
        'H' => NamedKey::H,
        'I' => NamedKey::I,
        'J' => NamedKey::J,
        'K' => NamedKey::K,
        'L' => NamedKey::L,
        'M' => NamedKey::M,
        'N' => NamedKey::N,
        'O' => NamedKey::O,
        'P' => NamedKey::P,
        'Q' => NamedKey::Q,
        'R' => NamedKey::R,
        'S' => NamedKey::S,
        'T' => NamedKey::T,
        'U' => NamedKey::U,
        'V' => NamedKey::V,
        'W' => NamedKey::W,
        'X' => NamedKey::X,
        'Y' => NamedKey::Y,
        'Z' => NamedKey::Z,
        _ => return None,
    })
}

fn digit(ch: char) -> Option<NamedKey> {
    Some(match ch {
        '0' => NamedKey::Digit0,
        '1' => NamedKey::Digit1,
        '2' => NamedKey::Digit2,
        '3' => NamedKey::Digit3,
        '4' => NamedKey::Digit4,
        '5' => NamedKey::Digit5,
        '6' => NamedKey::Digit6,
        '7' => NamedKey::Digit7,
        '8' => NamedKey::Digit8,
        '9' => NamedKey::Digit9,
        _ => return None,
    })
}

/// A key the hook can report. Side-specific modifiers stay distinct so Right
/// Alt does not match Left Alt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PhysicalKey {
    LCtrl,
    RCtrl,
    LAlt,
    RAlt,
    LShift,
    RShift,
    LWin,
    RWin,
    Named(NamedKey),
    Other(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordPart {
    Ctrl { side: Option<Side> },
    Alt { side: Option<Side> },
    Shift { side: Option<Side> },
    Win { side: Option<Side> },
    Key(NamedKey),
}

impl ChordPart {
    fn satisfied_by(self, down: &BTreeSet<PhysicalKey>) -> bool {
        match self {
            Self::Ctrl { side: None } => {
                down.contains(&PhysicalKey::LCtrl) || down.contains(&PhysicalKey::RCtrl)
            }
            Self::Ctrl {
                side: Some(Side::Left),
            } => down.contains(&PhysicalKey::LCtrl),
            Self::Ctrl {
                side: Some(Side::Right),
            } => down.contains(&PhysicalKey::RCtrl),
            Self::Alt { side: None } => {
                down.contains(&PhysicalKey::LAlt) || down.contains(&PhysicalKey::RAlt)
            }
            Self::Alt {
                side: Some(Side::Left),
            } => down.contains(&PhysicalKey::LAlt),
            Self::Alt {
                side: Some(Side::Right),
            } => down.contains(&PhysicalKey::RAlt),
            Self::Shift { side: None } => {
                down.contains(&PhysicalKey::LShift) || down.contains(&PhysicalKey::RShift)
            }
            Self::Shift {
                side: Some(Side::Left),
            } => down.contains(&PhysicalKey::LShift),
            Self::Shift {
                side: Some(Side::Right),
            } => down.contains(&PhysicalKey::RShift),
            Self::Win { side: None } => {
                down.contains(&PhysicalKey::LWin) || down.contains(&PhysicalKey::RWin)
            }
            Self::Win {
                side: Some(Side::Left),
            } => down.contains(&PhysicalKey::LWin),
            Self::Win {
                side: Some(Side::Right),
            } => down.contains(&PhysicalKey::RWin),
            Self::Key(named) => down.contains(&PhysicalKey::Named(named)),
        }
    }

    fn matches_key(self, key: PhysicalKey) -> bool {
        match (self, key) {
            (Self::Ctrl { side: None }, PhysicalKey::LCtrl | PhysicalKey::RCtrl) => true,
            (
                Self::Ctrl {
                    side: Some(Side::Left),
                },
                PhysicalKey::LCtrl,
            ) => true,
            (
                Self::Ctrl {
                    side: Some(Side::Right),
                },
                PhysicalKey::RCtrl,
            ) => true,
            (Self::Alt { side: None }, PhysicalKey::LAlt | PhysicalKey::RAlt) => true,
            (
                Self::Alt {
                    side: Some(Side::Left),
                },
                PhysicalKey::LAlt,
            ) => true,
            (
                Self::Alt {
                    side: Some(Side::Right),
                },
                PhysicalKey::RAlt,
            ) => true,
            (Self::Shift { side: None }, PhysicalKey::LShift | PhysicalKey::RShift) => true,
            (
                Self::Shift {
                    side: Some(Side::Left),
                },
                PhysicalKey::LShift,
            ) => true,
            (
                Self::Shift {
                    side: Some(Side::Right),
                },
                PhysicalKey::RShift,
            ) => true,
            (Self::Win { side: None }, PhysicalKey::LWin | PhysicalKey::RWin) => true,
            (
                Self::Win {
                    side: Some(Side::Left),
                },
                PhysicalKey::LWin,
            ) => true,
            (
                Self::Win {
                    side: Some(Side::Right),
                },
                PhysicalKey::RWin,
            ) => true,
            (Self::Key(named), PhysicalKey::Named(key)) => named == key,
            _ => false,
        }
    }

    fn label(self) -> String {
        match self {
            Self::Ctrl { side: None } => "Ctrl".into(),
            Self::Ctrl {
                side: Some(Side::Left),
            } => "Left Ctrl".into(),
            Self::Ctrl {
                side: Some(Side::Right),
            } => "Right Ctrl".into(),
            Self::Alt { side: None } => "Alt".into(),
            Self::Alt {
                side: Some(Side::Left),
            } => "Left Alt".into(),
            Self::Alt {
                side: Some(Side::Right),
            } => "Right Alt".into(),
            Self::Shift { side: None } => "Shift".into(),
            Self::Shift {
                side: Some(Side::Left),
            } => "Left Shift".into(),
            Self::Shift {
                side: Some(Side::Right),
            } => "Right Shift".into(),
            Self::Win { side: None } => "Win".into(),
            Self::Win {
                side: Some(Side::Left),
            } => "Left Win".into(),
            Self::Win {
                side: Some(Side::Right),
            } => "Right Win".into(),
            Self::Key(key) => key.label().into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub parts: Vec<ChordPart>,
}

impl Binding {
    pub fn parse(input: &str) -> Result<Self, HotkeyParseError> {
        let tokens: Vec<&str> = input
            .split('+')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .collect();
        if tokens.is_empty() {
            return Err(HotkeyParseError::Empty);
        }
        let mut parts = Vec::with_capacity(tokens.len());
        for token in tokens {
            parts.push(
                parse_part(token)
                    .ok_or_else(|| HotkeyParseError::UnknownToken(token.to_string()))?,
            );
        }
        Ok(Self { parts })
    }

    pub fn right_alt() -> Self {
        Self {
            parts: vec![ChordPart::Alt {
                side: Some(Side::Right),
            }],
        }
    }

    pub fn ctrl_win() -> Self {
        Self {
            parts: vec![
                ChordPart::Ctrl { side: None },
                ChordPart::Win { side: None },
            ],
        }
    }

    pub fn is_active(&self, down: &BTreeSet<PhysicalKey>) -> bool {
        self.parts.iter().all(|part| part.satisfied_by(down))
    }

    pub fn involves(&self, key: PhysicalKey) -> bool {
        self.parts.iter().any(|part| part.matches_key(key))
    }

    pub fn display(&self) -> String {
        self.parts
            .iter()
            .map(|part| part.label())
            .collect::<Vec<_>>()
            .join("+")
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

fn parse_part(token: &str) -> Option<ChordPart> {
    let normalized = token.to_ascii_lowercase().replace([' ', '_'], "");
    match normalized.as_str() {
        "ctrl" | "control" => Some(ChordPart::Ctrl { side: None }),
        "lctrl" | "leftctrl" | "lcontrol" => Some(ChordPart::Ctrl {
            side: Some(Side::Left),
        }),
        "rctrl" | "rightctrl" | "rcontrol" => Some(ChordPart::Ctrl {
            side: Some(Side::Right),
        }),
        "alt" | "menu" => Some(ChordPart::Alt { side: None }),
        "lalt" | "leftalt" => Some(ChordPart::Alt {
            side: Some(Side::Left),
        }),
        "ralt" | "rightalt" | "altgr" => Some(ChordPart::Alt {
            side: Some(Side::Right),
        }),
        "shift" => Some(ChordPart::Shift { side: None }),
        "lshift" | "leftshift" => Some(ChordPart::Shift {
            side: Some(Side::Left),
        }),
        "rshift" | "rightshift" => Some(ChordPart::Shift {
            side: Some(Side::Right),
        }),
        "win" | "meta" | "super" | "windows" => Some(ChordPart::Win { side: None }),
        "lwin" | "leftwin" => Some(ChordPart::Win {
            side: Some(Side::Left),
        }),
        "rwin" | "rightwin" => Some(ChordPart::Win {
            side: Some(Side::Right),
        }),
        _ => NamedKey::parse(&normalized).map(ChordPart::Key),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyParseError {
    #[error("hotkey is empty")]
    Empty,
    #[error("unknown hotkey token `{0}`")]
    UnknownToken(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: PhysicalKey,
    pub pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub edge: Option<Edge>,
    pub swallow: bool,
}

#[derive(Debug, Clone)]
pub struct ChordTracker {
    binding: Binding,
    down: BTreeSet<PhysicalKey>,
    active: bool,
}

impl ChordTracker {
    pub fn new(binding: Binding) -> Self {
        Self {
            binding,
            down: BTreeSet::new(),
            active: false,
        }
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    pub fn set_binding(&mut self, binding: Binding) {
        if binding != self.binding {
            self.binding = binding;
            self.down.clear();
            self.active = false;
        }
    }

    pub fn on_event(&mut self, event: KeyEvent) -> Decision {
        if event.pressed {
            self.down.insert(event.key);
        } else {
            self.down.remove(&event.key);
        }
        let now_active = self.binding.is_active(&self.down);
        let edge = match (self.active, now_active) {
            (false, true) => Some(Edge::Pressed),
            (true, false) => Some(Edge::Released),
            _ => None,
        };
        let swallow = self.should_swallow(event, now_active);
        self.active = now_active;
        Decision { edge, swallow }
    }

    fn should_swallow(&self, event: KeyEvent, now_active: bool) -> bool {
        if !self.binding.involves(event.key) {
            return false;
        }
        if event.pressed {
            // Swallow the key that completes the chord, and repeats while it is held.
            now_active
        } else {
            // Swallow the release of a key that was part of an active chord.
            self.active
        }
    }
}

/// Map a chord edge to a session start/stop, given the activation mode and
/// whether a session is already open.
pub fn activation_command(mode: Activation, edge: Edge, session_open: bool) -> Option<bool> {
    match (mode, edge) {
        (Activation::PushToTalk, Edge::Pressed) if !session_open => Some(true),
        (Activation::PushToTalk, Edge::Released) if session_open => Some(false),
        (Activation::Toggle, Edge::Pressed) => Some(!session_open),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(tracker: &mut ChordTracker, key: PhysicalKey) -> Decision {
        tracker.on_event(KeyEvent { key, pressed: true })
    }

    fn release(tracker: &mut ChordTracker, key: PhysicalKey) -> Decision {
        tracker.on_event(KeyEvent {
            key,
            pressed: false,
        })
    }

    #[test]
    fn right_alt_press_and_release() {
        let mut tracker = ChordTracker::new(Binding::parse("RAlt").unwrap());
        let down = press(&mut tracker, PhysicalKey::RAlt);
        assert_eq!(down.edge, Some(Edge::Pressed));
        assert!(down.swallow);
        let repeat = press(&mut tracker, PhysicalKey::RAlt);
        assert_eq!(repeat.edge, None);
        assert!(repeat.swallow);
        let up = release(&mut tracker, PhysicalKey::RAlt);
        assert_eq!(up.edge, Some(Edge::Released));
        assert!(up.swallow);
    }

    #[test]
    fn left_alt_does_not_trigger_right_alt() {
        let mut tracker = ChordTracker::new(Binding::right_alt());
        let decision = press(&mut tracker, PhysicalKey::LAlt);
        assert_eq!(decision.edge, None);
        assert!(!decision.swallow);
    }

    #[test]
    fn ctrl_win_requires_both_and_does_not_swallow_ctrl_alone() {
        let mut tracker = ChordTracker::new(Binding::parse("Ctrl+Win").unwrap());
        let ctrl = press(&mut tracker, PhysicalKey::LCtrl);
        assert_eq!(ctrl.edge, None);
        assert!(!ctrl.swallow, "Ctrl alone must still reach the focused app");

        let win = press(&mut tracker, PhysicalKey::LWin);
        assert_eq!(win.edge, Some(Edge::Pressed));
        assert!(win.swallow, "Win is swallowed once the chord is complete");

        let other = press(&mut tracker, PhysicalKey::Named(NamedKey::Left));
        assert_eq!(other.edge, None);
        assert!(!other.swallow);

        let win_up = release(&mut tracker, PhysicalKey::LWin);
        assert_eq!(win_up.edge, Some(Edge::Released));
        assert!(win_up.swallow);
    }

    #[test]
    fn win_alone_does_not_activate_ctrl_win() {
        let mut tracker = ChordTracker::new(Binding::ctrl_win());
        let decision = press(&mut tracker, PhysicalKey::RWin);
        assert_eq!(decision.edge, None);
        assert!(!decision.swallow);
    }

    #[test]
    fn either_side_satisfies_a_generic_modifier() {
        let mut tracker = ChordTracker::new(Binding::parse("Ctrl+Win").unwrap());
        press(&mut tracker, PhysicalKey::RCtrl);
        let decision = press(&mut tracker, PhysicalKey::RWin);
        assert_eq!(decision.edge, Some(Edge::Pressed));
    }

    #[test]
    fn parse_display_and_reject_unknown() {
        let binding = Binding::parse("ctrl+shift+space").unwrap();
        assert_eq!(binding.display(), "Ctrl+Shift+Space");
        assert!(Binding::parse("").is_err());
        assert!(Binding::parse("Ctrl+NotAKey").is_err());
        assert_eq!(Binding::parse("RAlt").unwrap(), Binding::right_alt());
    }

    #[test]
    fn toggle_uses_press_only_and_ptt_uses_both_edges() {
        assert_eq!(
            activation_command(Activation::PushToTalk, Edge::Pressed, false),
            Some(true)
        );
        assert_eq!(
            activation_command(Activation::PushToTalk, Edge::Released, true),
            Some(false)
        );
        assert_eq!(
            activation_command(Activation::PushToTalk, Edge::Pressed, true),
            None
        );
        assert_eq!(
            activation_command(Activation::Toggle, Edge::Pressed, false),
            Some(true)
        );
        assert_eq!(
            activation_command(Activation::Toggle, Edge::Pressed, true),
            Some(false)
        );
        assert_eq!(
            activation_command(Activation::Toggle, Edge::Released, true),
            None
        );
    }

    #[test]
    fn changing_binding_resets_held_keys() {
        let mut tracker = ChordTracker::new(Binding::right_alt());
        press(&mut tracker, PhysicalKey::RAlt);
        tracker.set_binding(Binding::ctrl_win());
        let decision = press(&mut tracker, PhysicalKey::LWin);
        assert_eq!(decision.edge, None);
    }
}
