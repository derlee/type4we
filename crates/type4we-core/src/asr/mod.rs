//! Pluggable streaming speech recognition.
//!
//! A future on-device engine implements the same [`AsrEngine`] trait and
//! reports its own [`AudioRequirements`]. The session driver resamples to
//! whatever the active engine asks for, so adding a local engine does not
//! change hotkey, capture, or injection code.

mod drive;
mod soniox;
mod volcengine;

pub use drive::{drive_session, AudioChunk, DriveSession};
pub use soniox::{
    parse_server_message, start_message, SonioxEngine, SonioxOptions, Token, TranscriptBuilder,
    SONIOX_WS_URL,
};
pub use volcengine::{
    known_resource_ids, VolcengineEngine, RESOURCE_BIG_CONCURRENT, RESOURCE_BIG_DURATION,
    RESOURCE_SEED_CONCURRENT, RESOURCE_SEED_DURATION, VOLC_WS_URL,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioRequirements {
    pub sample_rate: u32,
    pub channels: u16,
    /// Outbound packet duration. Volcengine's realtime docs recommend 200 ms.
    pub frame_ms: u32,
}

impl AudioRequirements {
    pub const PCM_S16LE_16K_MONO: Self = Self {
        sample_rate: 16_000,
        channels: 1,
        frame_ms: 100,
    };

    /// Samples in one outbound packet.
    pub fn frame_samples(self) -> usize {
        let ms = if self.frame_ms == 0 {
            100
        } else {
            self.frame_ms
        };
        (self.sample_rate as usize * ms as usize / 1000).max(1)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AsrCredentials {
    pub values: std::collections::BTreeMap<String, String>,
}

impl AsrCredentials {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsrSessionOptions {
    pub model: String,
    pub language_hints: Vec<String>,
    pub endpoint_sensitivity: f64,
    pub resource_id: String,
    pub auth_mode: String,
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

    /// Optional idle ping. Soniox needs one; Volcengine stays quiet.
    async fn keepalive(&mut self) -> Result<(), AsrError> {
        Ok(())
    }
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
        "volcano" | "volcengine" => Ok(Box::new(VolcengineEngine)),
        "soniox" => Ok(Box::new(SonioxEngine)),
        other => Err(AsrError::UnknownProvider(other.to_string())),
    }
}
