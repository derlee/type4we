use std::sync::Arc;

use tokio::sync::mpsc;
use type4we_core::hotkey::Binding;
use type4we_core::inject::InjectionMethod;
use type4we_core::secrets::{MemorySecretStore, SecretStore};

use super::SharedState;

pub struct Capture {
    pub sample_rate: u32,
    pub audio: mpsc::Receiver<Vec<f32>>,
    pub release: std::sync::mpsc::Sender<()>,
}

pub fn start_capture(_device: Option<&str>) -> Result<Capture, String> {
    Err("microphone capture is implemented on Windows only".into())
}

pub fn list_input_devices() -> Vec<String> {
    Vec::new()
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    None
}

pub fn install_hotkey(_shared: SharedState) -> Result<(), String> {
    Err("the low-level keyboard hook is implemented on Windows only".into())
}

pub fn inject_text(
    _text: &str,
    _method: InjectionMethod,
    _binding: &Binding,
) -> Result<(), String> {
    Err("text injection is implemented on Windows only".into())
}

pub fn windows_secret_store() -> Arc<dyn SecretStore> {
    Arc::new(MemorySecretStore::new())
}
