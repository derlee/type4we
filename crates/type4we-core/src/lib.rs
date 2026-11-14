//! Pure logic for Type4We. Platform bindings (WASAPI, the low-level keyboard
//! hook, SendInput, Credential Manager) live in the Tauri crate and call into
//! these modules.

pub mod asr;
pub mod audio;
pub mod config;
pub mod hotkey;
pub mod inject;
pub mod secrets;
pub mod session;

pub use asr::{create_engine, AsrEngine, AsrError, AudioRequirements, EngineEvent};
pub use config::{Config, LoadOutcome};
pub use hotkey::{Activation, Binding, ChordTracker, Edge, KeyEvent, PhysicalKey};
pub use inject::{plan_strokes, InjectionMethod, Stroke};
pub use secrets::{credential_target, SecretStore};
pub use session::{Effect, Phase, Session, SessionEvent};
