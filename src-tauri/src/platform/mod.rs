//! OS bindings. Windows implementations are `cfg(windows)`. Stubs keep the
//! crate type-checkable on Linux, where only the core library is tested.

mod shared;

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod cursor;
#[cfg(windows)]
mod hotkey;
#[cfg(windows)]
mod inject;
#[cfg(windows)]
mod secrets;

#[cfg(not(windows))]
mod stubs;

pub use shared::SharedState;

#[cfg(windows)]
pub use audio::{list_input_devices, start_capture, Capture};
#[cfg(windows)]
pub use cursor::cursor_pos;
#[cfg(windows)]
pub use hotkey::install_hotkey;
#[cfg(windows)]
pub use inject::inject_text;
#[cfg(windows)]
pub use secrets::windows_secret_store;

#[cfg(not(windows))]
pub use stubs::*;
