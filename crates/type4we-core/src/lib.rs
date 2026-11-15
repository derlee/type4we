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

pub use asr::{
    create_engine, known_resource_ids, AsrEngine, AsrError, AudioRequirements, EngineEvent,
    RESOURCE_BIG_CONCURRENT, RESOURCE_BIG_DURATION, RESOURCE_SEED_CONCURRENT,
    RESOURCE_SEED_DURATION, VOLC_WS_URL,
};
pub use config::{Config, LoadOutcome};
pub use hotkey::{Activation, Binding, ChordTracker, Edge, KeyEvent, PhysicalKey};
pub use inject::{plan_strokes, InjectionMethod, Stroke};
pub use secrets::{credential_target, SecretStore};
pub use session::{Effect, Phase, Session, SessionEvent};
