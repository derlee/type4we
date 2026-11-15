//! Volcengine (火山引擎) Doubao streaming speech recognition.
//!
//! Protocol checked against the official bigmodel WebSocket docs on 2026-11-15:
//! - <https://www.volcengine.com/docs/6561/1354869>
//! - <https://docs.volcengine.com/docs/doubao-voice/large-model-streaming-automatic-speech-recognition-api-1>
//!
//! Real-time dictation uses the optimized bidirectional endpoint
//! `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async`. The docs say
//! this variant returns a frame only when the transcript changes and is the
//! one to prefer over `/bigmodel` and `/bigmodel_nostream`.
//!
//! Frames are big-endian. A 4-byte header is followed by an optional int32
//! sequence, a uint32 payload size, and the payload. Full-client and server
//! JSON payloads are gzip-compressed. Audio payloads are gzip-compressed
//! PCM s16le. The last audio packet uses message-type flags `0b0011` and a
//! negative sequence number (the "负包").
//!
//! Config keys follow Type4Me: `tf_asr_volcano` with `authMode`, `resourceId`,
//! `appKey`, `accessKey`, and `apiKey`.

use super::{
    AsrCredentials, AsrEngine, AsrError, AsrReader, AsrSessionOptions, AsrWriter,
    AudioRequirements, ServerEvent, Token,
};
use async_trait::async_trait;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    MaybeTlsStream, WebSocketStream,
};

pub const VOLC_WS_URL: &str = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";

pub const RESOURCE_SEED_DURATION: &str = "volc.seedasr.sauc.duration";
pub const RESOURCE_SEED_CONCURRENT: &str = "volc.seedasr.sauc.concurrent";
pub const RESOURCE_BIG_DURATION: &str = "volc.bigasr.sauc.duration";
pub const RESOURCE_BIG_CONCURRENT: &str = "volc.bigasr.sauc.concurrent";

pub fn known_resource_ids() -> &'static [&'static str] {
    &[
        RESOURCE_SEED_DURATION,
        RESOURCE_SEED_CONCURRENT,
        RESOURCE_BIG_DURATION,
        RESOURCE_BIG_CONCURRENT,
    ]
}

const VERSION: u8 = 0b0001;
const HEADER_UNITS: u8 = 0b0001;
const FULL_CLIENT: u8 = 0b0001;
const AUDIO_ONLY: u8 = 0b0010;
const FULL_SERVER: u8 = 0b1001;
const ERROR_SERVER: u8 = 0b1111;
const JSON_SERIAL: u8 = 0b0001;
const NO_SERIAL: u8 = 0b0000;
const GZIP: u8 = 0b0001;
const NO_COMPRESS: u8 = 0b0000;
const FLAG_SEQUENCE: u8 = 0b0001;
const FLAG_LAST: u8 = 0b0010;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolcUtterance {
    pub text: String,
    pub definite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VolcResult {
    pub text: String,
    pub utterances: Vec<VolcUtterance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedFrame {
    Server {
        sequence: Option<i32>,
        last: bool,
        result: VolcResult,
    },
    Error {
        code: u32,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VolcAccumulator {
    committed: String,
}

impl VolcAccumulator {
    /// Turn one server snapshot into text that the session can inject.
    ///
    /// `newly_final` is only the definite text we have not already emitted.
    /// `partial` is the still-changing tail. A final frame commits whatever
    /// remains. The server may revise a hypothesis; already-injected text is
    /// not rewound.
    pub fn apply(&mut self, result: &VolcResult, is_final: bool) -> (String, String) {
        let canonical = canonical_text(result);
        let confirmed = if is_final {
            canonical.clone()
        } else {
            definite_prefix(result, &canonical)
        };
        let stable = if confirmed.starts_with(&self.committed) {
            confirmed
        } else {
            self.committed.clone()
        };
        let newly = stable[self.committed.len()..].to_string();
        self.committed = stable;
        let partial = if is_final {
            String::new()
        } else if canonical.starts_with(&self.committed) {
            canonical[self.committed.len()..].to_string()
        } else {
            String::new()
        };
        (newly, partial)
    }
}

fn canonical_text(result: &VolcResult) -> String {
    let text = result.text.trim();
    if !text.is_empty() {
        return result.text.clone();
    }
    result
        .utterances
        .iter()
        .map(|utterance| utterance.text.as_str())
        .filter(|text| !text.is_empty())
        .collect()
}

fn definite_prefix(result: &VolcResult, canonical: &str) -> String {
    let mut rest = canonical;
    let mut taken = String::new();
    for utterance in result.utterances.iter().filter(|item| item.definite) {
        if utterance.text.is_empty() {
            continue;
        }
        let Some(stripped) = rest.strip_prefix(utterance.text.as_str()) else {
            break;
        };
        taken.push_str(&utterance.text);
        rest = stripped.trim_start_matches(' ');
    }
    taken
}

pub fn build_client_request() -> Value {
    // Language is omitted on purpose. The docs say an empty language uses the
    // default Chinese-and-English model. Forcing zh-CN would drop mixed English.
    json!({
        "user": { "uid": "type4we" },
        "audio": {
            "format": "pcm",
            "codec": "raw",
            "rate": 16000,
            "bits": 16,
            "channel": 1
        },
        "request": {
            "model_name": "bigmodel",
            "enable_itn": true,
            "enable_punc": true,
            "show_utterances": true,
            "enable_nonstream": true
        }
    })
}

pub fn auth_headers(
    credentials: &AsrCredentials,
    options: &AsrSessionOptions,
    connect_id: &str,
    request_id: &str,
) -> Result<Vec<(&'static str, String)>, AsrError> {
    let resource = if options.resource_id.trim().is_empty() {
        RESOURCE_SEED_DURATION.to_string()
    } else {
        options.resource_id.trim().to_string()
    };
    let mut headers = vec![
        ("X-Api-Resource-Id", resource),
        ("X-Api-Request-Id", request_id.to_string()),
        ("X-Api-Connect-Id", connect_id.to_string()),
        ("X-Api-Sequence", "-1".to_string()),
    ];
    match options.auth_mode.trim() {
        "apiKey" => {
            let key = credentials
                .get("apiKey")
                .ok_or(AsrError::MissingCredentials)?;
            headers.insert(0, ("X-Api-Key", key.to_string()));
        }
        _ => {
            let app = credentials
                .get("appKey")
                .ok_or(AsrError::MissingCredentials)?;
            let token = credentials
                .get("accessKey")
                .ok_or(AsrError::MissingCredentials)?;
            headers.insert(0, ("X-Api-Access-Key", token.to_string()));
            headers.insert(0, ("X-Api-App-Key", app.to_string()));
        }
    }
    Ok(headers)
}

pub fn encode_full_client_request(body: &Value) -> Result<Vec<u8>, AsrError> {
    let json = serde_json::to_vec(body).map_err(|err| AsrError::Protocol(err.to_string()))?;
    let payload = gzip(&json)?;
    // Official full-client example: flags 0b0000, so there is no sequence word.
    // Header bytes are 0x11 0x10 0x11 0x00.
    let mut frame = header(FULL_CLIENT, 0, JSON_SERIAL, GZIP);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn encode_audio(pcm: &[u8], sequence: i32, last: bool) -> Result<Vec<u8>, AsrError> {
    let payload = gzip(pcm)?;
    let (flags, seq) = if last {
        (FLAG_SEQUENCE | FLAG_LAST, -sequence.abs())
    } else {
        (FLAG_SEQUENCE, sequence)
    };
    let mut frame = header(AUDIO_ONLY, flags, NO_SERIAL, GZIP);
    frame.extend_from_slice(&seq.to_be_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_frame(bytes: &[u8]) -> Result<DecodedFrame, AsrError> {
    if bytes.len() < 4 {
        return Err(AsrError::Protocol(
            "volcengine frame is shorter than a header".into(),
        ));
    }
    let header_bytes = usize::from(bytes[0] & 0x0f) * 4;
    if header_bytes < 4 || bytes.len() < header_bytes {
        return Err(AsrError::Protocol(
            "volcengine header size is invalid".into(),
        ));
    }
    let message_type = bytes[1] >> 4;
    let flags = bytes[1] & 0x0f;
    let compression = bytes[2] & 0x0f;
    let mut rest = &bytes[header_bytes..];
    match message_type {
        ERROR_SERVER => {
            if rest.len() < 8 {
                return Err(AsrError::Protocol(
                    "volcengine error frame is truncated".into(),
                ));
            }
            let code = u32::from_be_bytes(rest[..4].try_into().unwrap());
            let size = u32::from_be_bytes(rest[4..8].try_into().unwrap()) as usize;
            rest = &rest[8..];
            let raw = rest.get(..size).unwrap_or(rest);
            let message = inflate_message(raw, compression)?;
            Ok(DecodedFrame::Error {
                code,
                message: explain_error(code, &message),
            })
        }
        FULL_SERVER => {
            let mut sequence = None;
            if flags & FLAG_SEQUENCE != 0 {
                if rest.len() < 4 {
                    return Err(AsrError::Protocol(
                        "volcengine response is missing its sequence".into(),
                    ));
                }
                sequence = Some(i32::from_be_bytes(rest[..4].try_into().unwrap()));
                rest = &rest[4..];
            }
            if rest.len() < 4 {
                return Err(AsrError::Protocol(
                    "volcengine response is missing its payload size".into(),
                ));
            }
            let size = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
            rest = &rest[4..];
            let raw = rest.get(..size).unwrap_or(rest);
            let text = inflate_message(raw, compression)?;
            let result = if text.trim().is_empty() {
                VolcResult::default()
            } else {
                parse_result_json(&text)?
            };
            let last = flags & FLAG_LAST != 0 || sequence.is_some_and(|value| value < 0);
            Ok(DecodedFrame::Server {
                sequence,
                last,
                result,
            })
        }
        other => Err(AsrError::Protocol(format!(
            "unexpected volcengine message type {other:#x}"
        ))),
    }
}

pub fn explain_error(code: u32, message: &str) -> String {
    let meaning = match code {
        20000000 => "success",
        45000001 => "invalid request parameters",
        45000002 => "empty audio",
        45000081 => "timed out waiting for the next audio packet",
        45000151 => "unsupported audio format",
        55000031 => "server busy",
        55_000_000..=55_999_999 => "internal server error",
        _ => "volcengine error",
    };
    let detail = message.trim();
    if detail.is_empty() {
        format!("Volcengine {code}: {meaning}")
    } else {
        format!("Volcengine {code}: {meaning} ({detail})")
    }
}

fn parse_result_json(text: &str) -> Result<VolcResult, AsrError> {
    let value: Value = serde_json::from_str(text)
        .map_err(|err| AsrError::Protocol(format!("volcengine json: {err}")))?;
    let result_value = match &value["result"] {
        Value::Array(items) => items.first().cloned().unwrap_or(Value::Null),
        other => other.clone(),
    };
    if result_value.is_null() {
        return Ok(VolcResult::default());
    }
    let parsed: RawResult = serde_json::from_value(result_value)
        .map_err(|err| AsrError::Protocol(format!("volcengine result: {err}")))?;
    Ok(VolcResult {
        text: parsed.text.unwrap_or_default(),
        utterances: parsed
            .utterances
            .unwrap_or_default()
            .into_iter()
            .map(|item| VolcUtterance {
                text: item.text.unwrap_or_default(),
                definite: item.definite.unwrap_or(false),
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
struct RawResult {
    text: Option<String>,
    utterances: Option<Vec<RawUtterance>>,
}

#[derive(Debug, Deserialize)]
struct RawUtterance {
    text: Option<String>,
    definite: Option<bool>,
}

fn header(message_type: u8, flags: u8, serial: u8, compression: u8) -> Vec<u8> {
    vec![
        (VERSION << 4) | HEADER_UNITS,
        (message_type << 4) | flags,
        (serial << 4) | compression,
        0x00,
    ]
}

fn gzip(bytes: &[u8]) -> Result<Vec<u8>, AsrError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .map_err(|err| AsrError::Protocol(err.to_string()))?;
    encoder
        .finish()
        .map_err(|err| AsrError::Protocol(err.to_string()))
}

fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, AsrError> {
    let mut decoder = GzDecoder::new(bytes);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|err| AsrError::Protocol(format!("gzip: {err}")))?;
    Ok(out)
}

fn inflate_message(bytes: &[u8], compression: u8) -> Result<String, AsrError> {
    let raw = if compression == GZIP {
        gunzip(bytes)?
    } else if compression == NO_COMPRESS {
        bytes.to_vec()
    } else {
        return Err(AsrError::Protocol(format!(
            "unsupported volcengine compression {compression:#x}"
        )));
    };
    String::from_utf8(raw).map_err(|err| AsrError::Protocol(format!("volcengine text: {err}")))
}

fn tokens_from(newly_final: &str, partial: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    if !newly_final.is_empty() {
        tokens.push(Token {
            text: newly_final.to_string(),
            is_final: true,
        });
    }
    if !partial.is_empty() {
        tokens.push(Token {
            text: partial.to_string(),
            is_final: false,
        });
    }
    tokens
}

pub struct VolcengineEngine;

#[async_trait]
impl AsrEngine for VolcengineEngine {
    fn id(&self) -> &'static str {
        "volcano"
    }

    fn audio_requirements(&self) -> AudioRequirements {
        AudioRequirements {
            sample_rate: 16_000,
            channels: 1,
            frame_ms: 200,
        }
    }

    async fn open(
        &self,
        credentials: &AsrCredentials,
        options: &AsrSessionOptions,
    ) -> Result<(Box<dyn AsrWriter>, Box<dyn AsrReader>), AsrError> {
        let connect_id = uuid::Uuid::new_v4().to_string();
        let request_id = uuid::Uuid::new_v4().to_string();
        let headers = auth_headers(credentials, options, &connect_id, &request_id)?;
        let mut request = VOLC_WS_URL
            .into_client_request()
            .map_err(|err| AsrError::WebSocket(err.to_string()))?;
        for (name, value) in headers {
            let header = HeaderValue::from_str(&value)
                .map_err(|err| AsrError::WebSocket(err.to_string()))?;
            request.headers_mut().insert(name, header);
        }
        let (socket, _response) = connect_async(request)
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))?;
        let (mut write, read) = socket.split();
        let start = encode_full_client_request(&build_client_request())?;
        write
            .send(Message::Binary(start.into()))
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))?;
        Ok((
            Box::new(VolcWriter {
                write,
                next_seq: 1,
                finished: false,
            }),
            Box::new(VolcReader {
                read,
                accumulator: VolcAccumulator::default(),
            }),
        ))
    }
}

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct VolcWriter {
    write: futures_util::stream::SplitSink<Ws, Message>,
    next_seq: i32,
    finished: bool,
}

struct VolcReader {
    read: futures_util::stream::SplitStream<Ws>,
    accumulator: VolcAccumulator,
}

#[async_trait]
impl AsrWriter for VolcWriter {
    async fn send_pcm(&mut self, pcm_s16le: &[u8]) -> Result<(), AsrError> {
        if self.finished || pcm_s16le.is_empty() {
            return Ok(());
        }
        let frame = encode_audio(pcm_s16le, self.next_seq, false)?;
        self.next_seq = self.next_seq.saturating_add(1);
        self.write
            .send(Message::Binary(frame.into()))
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))
    }

    async fn send_control(&mut self, _json: &str) -> Result<(), AsrError> {
        Ok(())
    }

    async fn finish(&mut self) -> Result<(), AsrError> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        let frame = encode_audio(&[], self.next_seq, true)?;
        self.write
            .send(Message::Binary(frame.into()))
            .await
            .map_err(|err| AsrError::WebSocket(err.to_string()))
    }
}

#[async_trait]
impl AsrReader for VolcReader {
    async fn recv(&mut self) -> Result<Option<ServerEvent>, AsrError> {
        loop {
            match tokio::time::timeout(Duration::from_secs(30), self.read.next()).await {
                Ok(Some(Ok(Message::Binary(bytes)))) => {
                    return self.event_from_frame(&bytes).map(Some);
                }
                Ok(Some(Ok(Message::Text(text)))) => {
                    return Err(AsrError::Protocol(format!(
                        "volcengine sent a text frame: {}",
                        text.chars().take(120).collect::<String>()
                    )));
                }
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) => return Ok(None),
                Ok(Some(Ok(_))) => continue,
                Ok(Some(Err(err))) => return Err(AsrError::WebSocket(err.to_string())),
                Err(_) => {
                    return Err(AsrError::WebSocket(
                        "timed out waiting for a Volcengine message".into(),
                    ))
                }
            }
        }
    }
}

impl VolcReader {
    fn event_from_frame(&mut self, bytes: &[u8]) -> Result<ServerEvent, AsrError> {
        match decode_frame(bytes)? {
            DecodedFrame::Error { code, message } => Ok(ServerEvent::Error {
                code: i64::from(code),
                error_type: Some("volcengine".into()),
                message,
            }),
            DecodedFrame::Server { last, result, .. } => {
                let (newly, partial) = self.accumulator.apply(&result, last);
                Ok(ServerEvent::Result {
                    tokens: tokens_from(&newly, &partial),
                    finished: last,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result_json() -> &'static str {
        r#"{"audio_info":{"duration":3696},"result":{"text":"这是字节跳动， 今日头条母公司。","utterances":[{"definite":true,"start_time":0,"end_time":1705,"text":"这是字节跳动，"},{"definite":false,"start_time":2110,"end_time":3696,"text":"今日头条母公司。"}]}}"#
    }

    #[test]
    fn full_client_request_header_and_gzip_json() {
        let frame = encode_full_client_request(&build_client_request()).unwrap();
        assert_eq!(&frame[..4], &[0x11, 0x10, 0x11, 0x00]);
        let size = u32::from_be_bytes(frame[4..8].try_into().unwrap()) as usize;
        assert_eq!(frame.len(), 8 + size);
        let json = gunzip(&frame[8..]).unwrap();
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["audio"]["format"], "pcm");
        assert_eq!(value["audio"]["rate"], 16000);
        assert_eq!(value["audio"]["bits"], 16);
        assert_eq!(value["audio"]["channel"], 1);
        assert!(value["audio"].get("language").is_none());
        assert_eq!(value["request"]["model_name"], "bigmodel");
        assert_eq!(value["request"]["enable_punc"], true);
        assert_eq!(value["request"]["enable_itn"], true);
        assert_eq!(value["request"]["show_utterances"], true);
        assert_eq!(value["request"]["enable_nonstream"], true);
    }

    #[test]
    fn audio_packet_carries_sequence_and_last_packet_is_negative() {
        let pcm = [0u8, 1, 2, 3];
        let frame = encode_audio(&pcm, 2, false).unwrap();
        assert_eq!(&frame[..4], &[0x11, 0x21, 0x01, 0x00]);
        assert_eq!(i32::from_be_bytes(frame[4..8].try_into().unwrap()), 2);
        let size = u32::from_be_bytes(frame[8..12].try_into().unwrap()) as usize;
        assert_eq!(gunzip(&frame[12..12 + size]).unwrap(), pcm);

        let last = encode_audio(&[], 3, true).unwrap();
        assert_eq!(&last[..4], &[0x11, 0x23, 0x01, 0x00]);
        assert_eq!(i32::from_be_bytes(last[4..8].try_into().unwrap()), -3);
        let last_size = u32::from_be_bytes(last[8..12].try_into().unwrap()) as usize;
        assert!(gunzip(&last[12..12 + last_size]).unwrap().is_empty());
    }

    #[test]
    fn server_response_roundtrip_parses_partial_and_definite() {
        let payload = gzip(sample_result_json().as_bytes()).unwrap();
        let mut frame = header(FULL_SERVER, FLAG_SEQUENCE, JSON_SERIAL, GZIP);
        frame.extend_from_slice(&1i32.to_be_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&payload);
        let decoded = decode_frame(&frame).unwrap();
        match decoded {
            DecodedFrame::Server {
                sequence,
                last,
                result,
            } => {
                assert_eq!(sequence, Some(1));
                assert!(!last);
                assert!(result.text.contains("字节跳动"));
                assert!(result.utterances[0].definite);
                assert!(!result.utterances[1].definite);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn last_server_flag_and_error_frame() {
        let payload = gzip(br#"{"result":{"text":"done"}}"#).unwrap();
        let mut frame = header(FULL_SERVER, FLAG_SEQUENCE | FLAG_LAST, JSON_SERIAL, GZIP);
        frame.extend_from_slice(&(-4i32).to_be_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(&payload);
        match decode_frame(&frame).unwrap() {
            DecodedFrame::Server {
                last,
                sequence,
                result,
            } => {
                assert!(last);
                assert_eq!(sequence, Some(-4));
                assert_eq!(result.text, "done");
            }
            other => panic!("unexpected {other:?}"),
        }

        let message = br#"{"message":"invalid params"}"#;
        let mut error = header(ERROR_SERVER, 0, JSON_SERIAL, NO_COMPRESS);
        error.extend_from_slice(&45000001u32.to_be_bytes());
        error.extend_from_slice(&(message.len() as u32).to_be_bytes());
        error.extend_from_slice(message);
        match decode_frame(&error).unwrap() {
            DecodedFrame::Error { code, message } => {
                assert_eq!(code, 45_000_001);
                assert!(message.contains("invalid request parameters"));
                assert!(message.contains("invalid params"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn accumulator_emits_definite_text_once_and_keeps_the_tail_partial() {
        let result = parse_result_json(sample_result_json()).unwrap();
        let mut acc = VolcAccumulator::default();
        let (newly, partial) = acc.apply(&result, false);
        assert_eq!(newly, "这是字节跳动，");
        assert_eq!(partial, " 今日头条母公司。");
        let (again, partial) = acc.apply(&result, false);
        assert!(again.is_empty());
        assert_eq!(partial, " 今日头条母公司。");
        let (rest, partial) = acc.apply(&result, true);
        assert_eq!(rest, " 今日头条母公司。");
        assert!(partial.is_empty());
    }

    #[test]
    fn legacy_and_api_key_headers() {
        let mut legacy = AsrCredentials::default();
        legacy.values.insert("appKey".into(), "app-1".into());
        legacy.values.insert("accessKey".into(), "token-1".into());
        let options = AsrSessionOptions {
            model: String::new(),
            language_hints: Vec::new(),
            endpoint_sensitivity: 0.0,
            resource_id: RESOURCE_BIG_DURATION.into(),
            auth_mode: "legacy".into(),
        };
        let headers = auth_headers(&legacy, &options, "connect", "request").unwrap();
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "X-Api-App-Key" && v == "app-1"));
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "X-Api-Access-Key" && v == "token-1"));
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "X-Api-Resource-Id" && v == RESOURCE_BIG_DURATION));
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "X-Api-Connect-Id" && v == "connect"));
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "X-Api-Sequence" && v == "-1"));

        let mut keyed = AsrCredentials::default();
        keyed.values.insert("apiKey".into(), "new-key".into());
        let mut api_options = options.clone();
        api_options.auth_mode = "apiKey".into();
        api_options.resource_id.clear();
        let headers = auth_headers(&keyed, &api_options, "c", "r").unwrap();
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "X-Api-Key" && v == "new-key"));
        assert!(headers
            .iter()
            .any(|(k, v)| *k == "X-Api-Resource-Id" && v == RESOURCE_SEED_DURATION));
        assert!(auth_headers(&AsrCredentials::default(), &options, "c", "r").is_err());
        assert_eq!(known_resource_ids().len(), 4);
    }

    #[tokio::test]
    #[ignore = "needs VOLC_APP_KEY and VOLC_ACCESS_KEY; hits the live Volcengine endpoint"]
    async fn live_volcengine_accepts_a_short_silence() {
        let app = std::env::var("VOLC_APP_KEY").expect("VOLC_APP_KEY");
        let token = std::env::var("VOLC_ACCESS_KEY").expect("VOLC_ACCESS_KEY");
        let mut credentials = AsrCredentials::default();
        credentials.values.insert("appKey".into(), app);
        credentials.values.insert("accessKey".into(), token);
        let options = AsrSessionOptions {
            model: String::new(),
            language_hints: Vec::new(),
            endpoint_sensitivity: 0.0,
            resource_id: std::env::var("VOLC_RESOURCE_ID")
                .unwrap_or_else(|_| RESOURCE_SEED_DURATION.into()),
            auth_mode: "legacy".into(),
        };
        let engine = VolcengineEngine;
        let (mut writer, mut reader) = engine.open(&credentials, &options).await.unwrap();
        writer.send_pcm(&vec![0u8; 6400]).await.unwrap();
        writer.finish().await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(5), reader.recv()).await;
    }
}
