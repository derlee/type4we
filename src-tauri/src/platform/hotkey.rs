//! `WH_KEYBOARD_LL` hook. `RegisterHotKey` cannot express hold-to-talk: it
//! fires once on keydown and does not deliver a clean keyup for modifier-only
//! chords. The low-level hook sees both edges and the left/right variants.

use std::sync::OnceLock;

use type4we_core::hotkey::{KeyEvent, PhysicalKey};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, HC_ACTION,
    KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYUP, WM_SYSKEYUP,
};

use super::SharedState;
use crate::messages::CoordCommand;

static SHARED: OnceLock<SharedState> = OnceLock::new();

pub fn install_hotkey(shared: SharedState) -> Result<(), String> {
    SHARED
        .set(shared)
        .map_err(|_| "hotkey hook is already installed".to_string())?;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("type4we-hotkey".into())
        .spawn(move || unsafe {
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
                Ok(hook) => {
                    let _ = ready_tx.send(Ok(()));
                    let mut message = MSG::default();
                    while GetMessageW(&mut message, None, 0, 0).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                    let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(hook);
                }
                Err(err) => {
                    let _ = ready_tx.send(Err(err.to_string()));
                }
            }
        })
        .map_err(|err| err.to_string())?;
    ready_rx.recv().map_err(|err| err.to_string())?
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        if let Some(shared) = SHARED.get() {
            let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let key_up =
                wparam.0 == WM_KEYUP as usize || wparam.0 == WM_SYSKEYUP as usize || flag_up(info);
            let event = KeyEvent {
                key: vk_to_physical(info.vkCode),
                pressed: !key_up,
            };
            let mut tracker = shared.tracker.lock().unwrap_or_else(|err| err.into_inner());
            if let Ok(config) = shared.config.read() {
                if let Ok(binding) = config.binding() {
                    tracker.set_binding(binding);
                }
            }
            let decision = tracker.on_event(event);
            drop(tracker);
            if let Some(edge) = decision.edge {
                let _ = shared.commands.send(CoordCommand::Hotkey(edge));
            }
            if decision.swallow {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn flag_up(info: &KBDLLHOOKSTRUCT) -> bool {
    // LLKHF_UP is 0x80. The windows crate exposes the flags as a bitflags newtype.
    info.flags.0 & 0x80 != 0
}

fn vk_to_physical(vk: u32) -> PhysicalKey {
    match vk {
        0xA2 => PhysicalKey::LCtrl,
        0xA3 => PhysicalKey::RCtrl,
        0xA4 => PhysicalKey::LAlt,
        0xA5 => PhysicalKey::RAlt,
        0xA0 => PhysicalKey::LShift,
        0xA1 => PhysicalKey::RShift,
        0x5B => PhysicalKey::LWin,
        0x5C => PhysicalKey::RWin,
        0x11 => PhysicalKey::LCtrl,
        0x12 => PhysicalKey::LAlt,
        0x10 => PhysicalKey::LShift,
        code @ 0x41..=0x5A => PhysicalKey::Named(letter(code)),
        code @ 0x30..=0x39 => PhysicalKey::Named(digit(code)),
        0x70 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F1),
        0x71 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F2),
        0x72 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F3),
        0x73 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F4),
        0x74 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F5),
        0x75 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F6),
        0x76 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F7),
        0x77 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F8),
        0x78 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F9),
        0x79 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F10),
        0x7A => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F11),
        0x7B => PhysicalKey::Named(type4we_core::hotkey::NamedKey::F12),
        0x20 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Space),
        0x09 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Tab),
        0x0D => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Enter),
        0x1B => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Escape),
        0x25 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Left),
        0x27 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Right),
        0x26 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Up),
        0x28 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Down),
        0x24 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Home),
        0x23 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::End),
        0x21 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::PageUp),
        0x22 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::PageDown),
        0x2D => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Insert),
        0x2E => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Delete),
        0x13 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Pause),
        0x91 => PhysicalKey::Named(type4we_core::hotkey::NamedKey::Scroll),
        other => PhysicalKey::Other(other),
    }
}

fn letter(code: u32) -> type4we_core::hotkey::NamedKey {
    use type4we_core::hotkey::NamedKey::*;
    const LETTERS: [type4we_core::hotkey::NamedKey; 26] = [
        A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    ];
    LETTERS[(code - 0x41) as usize]
}

fn digit(code: u32) -> type4we_core::hotkey::NamedKey {
    use type4we_core::hotkey::NamedKey::*;
    const DIGITS: [type4we_core::hotkey::NamedKey; 10] = [
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    ];
    DIGITS[(code - 0x30) as usize]
}
