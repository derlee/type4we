//! Pluggable streaming speech recognition.
//!
//! A future on-device engine implements the same [`AsrEngine`] trait and
//! reports its own [`AudioRequirements`]. The session driver resamples to
//! whatever the active engine asks for, so adding a local engine does not
//! change hotkey, capture, or injection code.

mod drive;
mod soniox;

pub use drive::{drive_session, AudioChunk, DriveSession};
pub use soniox::{
    parse_server_message, start_message, SonioxEngine, SonioxOptions, Token, TranscriptBuilder,
    SONIOX_WS_URL,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioRequirements {
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioRequirements {
    pub const PCM_S16LE_16K_MONO: Self = Self {
        sample_rate: 16_000,
        channels: 1,
    };

    /// Samples in one outbound packet. 100 ms matches typical streaming ASR.
    pub fn frame_samples(self) -> usize {
        (self.sample_rate as usize / 10).max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrCredentials {
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsrSessionOptions {
    pub model: String,
    pub language_hints: Vec<String>,
    pub endpoint_sensitivity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    Result {
        tokens: Vec<Token>,
        finished: bool,
    },
    Error {
        code: i64,
        error_type: Option<String>,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    Ready,
    Update {
        newly_final: String,
        preview: String,
    },
    Finished {
        remainder: String,
    },
    Failed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("missing API key")]
    MissingCredentials,
    #[error("unknown ASR provider `{0}`")]
    UnknownProvider(String),
    #[error("websocket: {0}")]
    WebSocket(String),
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("{message}")]
    Provider {
        code: i64,
        error_type: Option<String>,
        message: String,
    },
}

#[async_trait]
pub trait AsrWriter: Send {
    async fn send_pcm(&mut self, pcm_s16le: &[u8]) -> Result<(), AsrError>;
    async fn send_control(&mut self, json: &str) -> Result<(), AsrError>;
    async fn finish(&mut self) -> Result<(), AsrError>;
}

#[async_trait]
pub trait AsrReader: Send {
    async fn recv(&mut self) -> Result<Option<ServerEvent>, AsrError>;
}

#[async_trait]
pub trait AsrEngine: Send + Sync {
    fn id(&self) -> &'static str;
    fn audio_requirements(&self) -> AudioRequirements;
    async fn open(
        &self,
        credentials: &AsrCredentials,
        options: &AsrSessionOptions,
    ) -> Result<(Box<dyn AsrWriter>, Box<dyn AsrReader>), AsrError>;
}

pub fn create_engine(id: &str) -> Result<Box<dyn AsrEngine>, AsrError> {
    match id {
        "soniox" => Ok(Box::new(SonioxEngine)),
        other => Err(AsrError::UnknownProvider(other.to_string())),
    }
}
