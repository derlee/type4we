//! `SendInput` with `KEYEVENTF_UNICODE`, then a clipboard Ctrl+V fallback.
//!
//! UIPI: an elevated window silently drops input from a medium-integrity
//! process. `SendInput` can still report success. Fixing that requires a
//! signed binary with `uiAccess=true`, which is out of scope for the MVP.

use std::mem::size_of;
use std::thread;
use std::time::Duration;

use type4we_core::hotkey::{Binding, PhysicalKey};
use type4we_core::inject::{plan_strokes, uses_clipboard, InjectionMethod, Stroke};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};

const CF_UNICODETEXT: u32 = 13;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_RETURN,
};

pub fn inject_text(text: &str, method: InjectionMethod, binding: &Binding) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    wait_until_chord_released(binding);
    let strokes = plan_strokes(text);
    if method != InjectionMethod::Clipboard {
        let inputs = strokes_to_inputs(&strokes);
        let requested = inputs.len() as u32;
        let inserted = send_inputs(&inputs);
        if !uses_clipboard(method, requested, inserted) {
            if inserted < requested {
                return Err(format!(
                    "SendInput delivered {inserted} of {requested} events. An elevated window will not accept input from this app (UIPI) unless it is signed with uiAccess."
                ));
            }
            return Ok(());
        }
        tracing::info!(
            "SendInput inserted {inserted} of {requested}; using the clipboard fallback"
        );
    }
    clipboard_paste(text)
}

fn strokes_to_inputs(strokes: &[Stroke]) -> Vec<INPUT> {
    let mut inputs = Vec::with_capacity(strokes.len() * 2);
    for stroke in strokes {
        match stroke {
            Stroke::Unicode(unit) => {
                inputs.push(unicode_event(*unit, false));
                inputs.push(unicode_event(*unit, true));
            }
            Stroke::Enter => {
                inputs.push(virtual_event(VK_RETURN, false));
                inputs.push(virtual_event(VK_RETURN, true));
            }
        }
    }
    inputs
}

fn send_inputs(inputs: &[INPUT]) -> u32 {
    let mut inserted = 0;
    for chunk in inputs.chunks(64) {
        inserted += unsafe { SendInput(chunk, size_of::<INPUT>() as i32) };
    }
    inserted
}

fn unicode_event(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn virtual_event(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn clipboard_paste(text: &str) -> Result<(), String> {
    let previous = read_unicode_clipboard()?;
    set_unicode_clipboard(text)?;
    let inputs = [
        virtual_event(VK_CONTROL, false),
        virtual_event(VIRTUAL_KEY(u16::from(b'V')), false),
        virtual_event(VIRTUAL_KEY(u16::from(b'V')), true),
        virtual_event(VK_CONTROL, true),
    ];
    let inserted = send_inputs(&inputs);
    // The target app reads the clipboard asynchronously. Restore after a short pause.
    thread::sleep(Duration::from_millis(100));
    let restored = match previous {
        Some(previous) => set_unicode_clipboard(&previous),
        None => clear_clipboard(),
    };
    if inserted < inputs.len() as u32 {
        return Err(
            "Ctrl+V was not delivered. If the focused window is running as administrator, Windows UIPI blocks input from this unsigned app.".into(),
        );
    }
    restored
}

fn read_unicode_clipboard() -> Result<Option<String>, String> {
    unsafe {
        OpenClipboard(None).map_err(|err| format!("open clipboard: {err}"))?;
        let handle = GetClipboardData(CF_UNICODETEXT);
        let text = match handle {
            Ok(handle) if !handle.is_invalid() => locked_text(HGLOBAL(handle.0)),
            _ => None,
        };
        let _ = CloseClipboard();
        Ok(text)
    }
}

unsafe fn locked_text(memory: HGLOBAL) -> Option<String> {
    let ptr = GlobalLock(memory) as *const u16;
    if ptr.is_null() {
        return None;
    }
    let mut units = Vec::new();
    let mut index = 0usize;
    while index < 1_000_000 {
        let unit = *ptr.add(index);
        if unit == 0 {
            break;
        }
        units.push(unit);
        index += 1;
    }
    let _ = GlobalUnlock(memory);
    Some(String::from_utf16_lossy(&units))
}

fn set_unicode_clipboard(text: &str) -> Result<(), String> {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    unsafe {
        OpenClipboard(None).map_err(|err| format!("open clipboard: {err}"))?;
        if let Err(err) = EmptyClipboard() {
            let _ = CloseClipboard();
            return Err(format!("empty clipboard: {err}"));
        }
        let memory = match GlobalAlloc(GMEM_MOVEABLE, units.len() * 2) {
            Ok(memory) => memory,
            Err(err) => {
                let _ = CloseClipboard();
                return Err(format!("clipboard alloc: {err}"));
            }
        };
        let ptr = GlobalLock(memory) as *mut u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err("clipboard lock failed".into());
        }
        std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
        let _ = GlobalUnlock(memory);
        if let Err(err) = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(memory.0))) {
            let _ = CloseClipboard();
            return Err(format!("set clipboard: {err}"));
        }
        CloseClipboard().map_err(|err| format!("close clipboard: {err}"))
    }
}

fn clear_clipboard() -> Result<(), String> {
    unsafe {
        OpenClipboard(None).map_err(|err| format!("open clipboard: {err}"))?;
        let result = EmptyClipboard()
            .map(|_| ())
            .map_err(|err| format!("clear clipboard: {err}"));
        let _ = CloseClipboard();
        result
    }
}

fn wait_until_chord_released(binding: &Binding) {
    let keys = binding_virtual_keys(binding);
    let deadline = std::time::Instant::now() + Duration::from_millis(200);
    while std::time::Instant::now() < deadline {
        let held = keys.iter().any(|key| key_down(*key));
        if !held {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn binding_virtual_keys(binding: &Binding) -> Vec<i32> {
    let mut keys = Vec::new();
    for part in &binding.parts {
        match part {
            type4we_core::hotkey::ChordPart::Ctrl { side } => {
                push_side(&mut keys, side, 0xA2, 0xA3)
            }
            type4we_core::hotkey::ChordPart::Alt { side } => push_side(&mut keys, side, 0xA4, 0xA5),
            type4we_core::hotkey::ChordPart::Shift { side } => {
                push_side(&mut keys, side, 0xA0, 0xA1)
            }
            type4we_core::hotkey::ChordPart::Win { side } => push_side(&mut keys, side, 0x5B, 0x5C),
            type4we_core::hotkey::ChordPart::Key(_) => {}
        }
    }
    let _ = PhysicalKey::LCtrl;
    keys
}

fn push_side(
    keys: &mut Vec<i32>,
    side: &Option<type4we_core::hotkey::Side>,
    left: i32,
    right: i32,
) {
    match side {
        Some(type4we_core::hotkey::Side::Left) => keys.push(left),
        Some(type4we_core::hotkey::Side::Right) => keys.push(right),
        None => {
            keys.push(left);
            keys.push(right);
        }
    }
}

fn key_down(vk: i32) -> bool {
    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(vk) < 0 }
}
