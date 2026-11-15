//! Soniox realtime speech-to-text.
//!
//! Verified against the public docs on 2026-11-14:
//! - endpoint `wss://stt-rt.soniox.com/transcribe-websocket`
//! - auth via `Authorization: Bearer <api_key>` (the start-message `api_key`
//!   field is deprecated and is intentionally omitted)
//! - model `stt-rt-v6` (`stt-rt-v5` still works until 2026-12-15 and is accepted
//!   when a Type4Me config asks for it)
//! - raw audio `pcm_s16le`, 16 kHz, mono, binary frames
//! - end the stream with an empty text frame (an empty binary frame does not)
//! - final tokens are emitted once; non-final tokens are replaced every message
//!
//! https://docs.soniox.com/api-reference/stt/websocket-api
//! https://docs.soniox.com/stt/rt/real-time-transcription
//! https://docs.soniox.com/stt/models

use super::{
    AsrCredentials, AsrEngine, AsrError, AsrReader, AsrSessionOptions, AsrWriter,
    AudioRequirements, ServerEvent,
};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    MaybeTlsStream, WebSocketStream,
};

pub const SONIOX_WS_URL: &str = "wss://stt-rt.soniox.com/transcribe-websocket";
const KEEPALIVE: &str = r#"{"type":"keepalive"}"#;
const FINALIZE: &str = r#"{"type":"finalize"}"#;

#[derive(Debug, Clone, PartialEq)]
pub struct SonioxOptions {
    pub model: String,
    pub language_hints: Vec<String>,
    pub endpoint_sensitivity: f64,
}

impl From<&AsrSessionOptions> for SonioxOptions {
    fn from(options: &AsrSessionOptions) -> Self {
        Self {
            model: options.model.clone(),
            language_hints: options.language_hints.clone(),
            endpoint_sensitivity: options.endpoint_sensitivity,
        }
    }
}

pub fn start_message(options: &SonioxOptions) -> Result<String, AsrError> {
    if options.model.trim().is_empty() {
        return Err(AsrError::Protocol("model is empty".into()));
    }
    let sensitivity = options.endpoint_sensitivity.clamp(-1.0, 1.0);
    let mut body = json!({
        "model": options.model,
        "audio_format": "pcm_s16le",
        "sample_rate": 16_000,
        "num_channels": 1,
        "enable_endpoint_detection": true,
        "endpoint_sensitivity": sensitivity,
    });
    if !options.language_hints.is_empty() {
        body["language_hints"] = json!(options.language_hints);
    }
    Ok(body.to_string())
}

pub fn keepalive_message() -> &'static str {
    KEEPALIVE
}

pub fn finalize_message() -> &'static str {
    FINALIZE
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub text: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TranscriptBuilder {
    finals: String,
    partial: String,
}

impl TranscriptBuilder {
    pub fn push(&mut self, tokens: &[Token]) -> String {
        let before = self.finals.len();
        self.partial.clear();
        for token in tokens {
            if token.is_final {
                self.finals.push_str(&token.text);
            } else {
                self.partial.push_str(&token.text);
            }
        }
        self.finals[before..].to_string()
    }

    pub fn preview(&self) -> String {
        let mut preview = self.finals.clone();
        preview.push_str(&self.partial);
        preview
    }

    pub fn take_partial(&mut self) -> String {
        std::mem::take(&mut self.partial)
    }

    pub fn finals(&self) -> &str {
        &self.finals
    }
}

#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(default)]
    tokens: Vec<RawToken>,
    #[serde(default)]
    finished: bool,
    error_code: Option<i64>,
    error_type: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawToken {
    #[serde(default)]
    text: String,
    #[serde(default)]
    is_final: bool,
}

pub fn parse_server_message(text: &str) -> Result<ServerEvent, AsrError> {
    let raw: RawMessage = serde_json::from_str(text)
        .map_err(|err| AsrError::Protocol(format!("invalid server message: {err}")))?;
    if let Some(code) = raw.error_code {
        return Ok(ServerEvent::Error {
            code,
            error_type: raw.error_type,
            message: raw
                .error_message
                .unwrap_or_else(|| format!("soniox error {code}")),
        });
    }
    Ok(ServerEvent::Result {
        tokens: raw
            .tokens
            .into_iter()
            .map(|token| Token {
                text: token.text,
                is_final: token.is_final,
            })
            .collect(),
        finished: raw.finished,
    })
}

pub struct SonioxEngine;

#[async_trait]
impl AsrEngine for SonioxEngine {
    fn id(&self) -> &'static str {
        "soniox"
    }

    fn audio_requirements(&self) -> AudioRequirements {
        AudioRequirements::PCM_S16LE_16K_MONO
    }

    async fn open(
        &self,
        credentials: &AsrCredentials,
        options: &AsrSessionOptions,
    ) -> Result<(Box<dyn AsrWriter>, Box<dyn AsrReader>), AsrError> {
        let Some(api_key) = credentials.get("apiKey") else {
            return Err(AsrError::MissingCredentials);
        };
        let start = start_message(&SonioxOptions::from(options))?;
        let mut request = SONIOX_WS_URL
            .into_client_request()
            .map_err(|err| AsrError::WebSocket(err.to_string()))?;
        let header = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|err| AsrError::WebSocket(err.to_string()))?;
        request.headers_mut().insert("Authorization", header);
        let (socket, _response) = connect_async(request)
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))?;
        let (mut write, read) = socket.split();
        write
            .send(Message::Text(start.into()))
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))?;
        Ok((
            Box::new(SonioxWriter { write }),
            Box::new(SonioxReader { read }),
        ))
    }
}

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct SonioxWriter {
    write: futures_util::stream::SplitSink<Ws, Message>,
}

struct SonioxReader {
    read: futures_util::stream::SplitStream<Ws>,
}

#[async_trait]
impl AsrWriter for SonioxWriter {
    async fn send_pcm(&mut self, pcm_s16le: &[u8]) -> Result<(), AsrError> {
        self.write
            .send(Message::Binary(pcm_s16le.to_vec().into()))
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))
    }

    async fn send_control(&mut self, json: &str) -> Result<(), AsrError> {
        self.write
            .send(Message::Text(json.to_string().into()))
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))
    }

    async fn finish(&mut self) -> Result<(), AsrError> {
        self.send_control(finalize_message()).await?;
        // An empty *text* frame ends the stream. An empty binary frame is just silence.
        self.send_control("").await
    }

    async fn keepalive(&mut self) -> Result<(), AsrError> {
        self.send_control(keepalive_message()).await
    }
}

#[async_trait]
impl AsrReader for SonioxReader {
    async fn recv(&mut self) -> Result<Option<ServerEvent>, AsrError> {
        loop {
            match tokio::time::timeout(Duration::from_secs(30), self.read.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    return parse_server_message(text.as_ref()).map(Some);
                }
                Ok(Some(Ok(Message::Binary(_)))) => continue,
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return Ok(None),
                Ok(Some(Ok(_))) => continue,
                Ok(Some(Err(err))) => return Err(AsrError::WebSocket(err.to_string())),
                Err(_) => {
                    return Err(AsrError::WebSocket(
                        "timed out waiting for a Soniox message".into(),
                    ))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_message_matches_the_documented_pcm_shape_and_omits_the_api_key() {
        let json = start_message(&SonioxOptions {
            model: "stt-rt-v6".into(),
            language_hints: vec!["zh".into(), "en".into()],
            endpoint_sensitivity: -0.3,
        })
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["model"], "stt-rt-v6");
        assert_eq!(value["audio_format"], "pcm_s16le");
        assert_eq!(value["sample_rate"], 16000);
        assert_eq!(value["num_channels"], 1);
        assert_eq!(value["enable_endpoint_detection"], true);
        assert_eq!(value["endpoint_sensitivity"], -0.3);
        assert_eq!(value["language_hints"], serde_json::json!(["zh", "en"]));
        assert!(value.get("api_key").is_none());
        assert_eq!(keepalive_message(), r#"{"type":"keepalive"}"#);
        assert_eq!(finalize_message(), r#"{"type":"finalize"}"#);
    }

    #[test]
    fn sensitivity_is_clamped_into_the_documented_range() {
        let json = start_message(&SonioxOptions {
            model: "stt-rt-v6".into(),
            language_hints: Vec::new(),
            endpoint_sensitivity: 4.0,
        })
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["endpoint_sensitivity"], 1.0);
        assert!(value.get("language_hints").is_none());
    }

    #[test]
    fn documented_token_evolution_builds_how_are_you_doing() {
        // Fixture transcribed from the official real-time transcription guide:
        // non-final tokens are replaced every message; final tokens arrive once.
        let frames = [
            r#"{"tokens":[{"text":"How","is_final":false},{"text":"'re","is_final":false}]}"#,
            r#"{"tokens":[{"text":"How","is_final":false},{"text":" ","is_final":false},{"text":"are","is_final":false}]}"#,
            r#"{"tokens":[{"text":"How","is_final":true},{"text":" ","is_final":true},{"text":"are","is_final":false},{"text":" ","is_final":false},{"text":"you","is_final":false}]}"#,
            r#"{"tokens":[{"text":"are","is_final":true},{"text":" ","is_final":true},{"text":"you","is_final":true},{"text":" ","is_final":false},{"text":"do","is_final":false},{"text":"ing","is_final":false},{"text":"?","is_final":false}]}"#,
            r#"{"tokens":[{"text":" ","is_final":true},{"text":"do","is_final":true},{"text":"ing","is_final":true},{"text":"?","is_final":true}]}"#,
        ];
        let mut builder = TranscriptBuilder::default();
        let mut previews = Vec::new();
        for frame in frames {
            let event = parse_server_message(frame).unwrap();
            let ServerEvent::Result { tokens, finished } = event else {
                panic!("expected result");
            };
            assert!(!finished);
            builder.push(&tokens);
            previews.push(builder.preview());
        }
        assert_eq!(
            previews,
            vec![
                "How're".to_string(),
                "How are".to_string(),
                "How are you".to_string(),
                "How are you doing?".to_string(),
                "How are you doing?".to_string(),
            ]
        );
        assert_eq!(builder.finals(), "How are you doing?");
        assert!(builder.take_partial().is_empty());
    }

    #[test]
    fn finished_and_error_frames() {
        let finished = parse_server_message(
            r#"{"tokens":[],"final_audio_proc_ms":1560,"total_audio_proc_ms":1680,"finished":true}"#,
        )
        .unwrap();
        assert_eq!(
            finished,
            ServerEvent::Result {
                tokens: Vec::new(),
                finished: true
            }
        );

        let error = parse_server_message(
            r#"{"tokens":[],"error_code":401,"error_type":"unauthenticated","error_message":"Incorrect API key provided."}"#,
        )
        .unwrap();
        assert_eq!(
            error,
            ServerEvent::Error {
                code: 401,
                error_type: Some("unauthenticated".into()),
                message: "Incorrect API key provided.".into(),
            }
        );
    }

    #[test]
    fn malformed_payload_is_a_protocol_error() {
        assert!(parse_server_message("[]").is_err());
    }
}
