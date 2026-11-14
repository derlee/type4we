# ADR 0002: Cloud-only ASR, behind a trait

[中文](../../zh/adr/0002-cloud-asr.md)

## Status

Accepted for the MVP. A local engine is deliberately not implemented.

## Context

Type4Me can run on-device models (sherpa-onnx and a Python sidecar). Those pulls in native builds, model files, and on NVIDIA machines a CUDA stack. The Windows MVP is supposed to stay near the Mac cloud edition's size and to ship one recognizer end to end.

The recognizer still has to be replaceable. Quick Mode, the hotkey, and injection should not know which vendor produced the tokens.

## Decision

The MVP speaks only to a cloud streaming API. Soniox is that API, chosen because its public WebSocket is JSON, authenticates with a bearer header, and documents partial tokens clearly. Volcano Engine's Doubao streaming API is a binary protocol with more than one console auth scheme; it stays a later provider. Docs were checked on 14 November 2026 against:

- <https://docs.soniox.com/api-reference/stt/websocket-api>
- <https://docs.soniox.com/stt/rt/real-time-transcription>
- <https://docs.soniox.com/stt/models>

The default model is `stt-rt-v6`. `stt-rt-v5` is accepted when present in config and is scheduled for removal on 15 December 2026.

The extension point is `AsrEngine::open`, which returns an `AsrWriter` and an `AsrReader`. The engine also reports `AudioRequirements`. The capture path resamples to that format. A future local engine implements the same traits and does not change the session machine.

No sherpa-onnx, CUDA, or Python sidecar is linked.

## Consequences

- Dictation needs a network and a Soniox key stored in Credential Manager.
- Protocol tests use recorded JSON fixtures, not a live account.
- Adding Volcano or a local model is a new engine plus config fields, not a rewrite of hotkey or injection.
