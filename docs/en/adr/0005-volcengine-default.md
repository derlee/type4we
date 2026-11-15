# ADR 0005: Volcengine is the default ASR

[中文](../../zh/adr/0005-volcengine-default.md)

## Status

Accepted. Supersedes the "Soniox is the only provider" part of [ADR 0002](0002-cloud-asr.md). The `AsrEngine` trait from that ADR is unchanged.

## Context

The MVP shipped with Soniox because its public API is a JSON WebSocket and was the faster protocol to prove. The repo owner wants the default cloud engine to be Volcengine's Doubao streaming speech recognition (豆包流式语音识别大模型), with Soniox kept as a choice.

## Decision

The default `tf_selectedASRProvider` is `volcano`, the same raw value Type4Me uses. Settings lists Volcengine first.

The client follows the official bigmodel binary protocol, checked on 15 November 2026:

- <https://www.volcengine.com/docs/6561/1354869>
- <https://docs.volcengine.com/docs/doubao-voice/large-model-streaming-automatic-speech-recognition-api-1>

Realtime dictation uses `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async`. The docs recommend that optimized bidirectional mode over `/bigmodel` (a response per packet) and `/bigmodel_nostream` (results only after 15 seconds or the last packet). Audio is 16 kHz mono PCM s16le, sent in 200 ms gzip frames, which the same page calls the best packet size for bidirectional streaming.

Each frame is big-endian: a 4-byte header, an optional int32 sequence, a uint32 payload size, and the payload. The full-client request is JSON, gzip, and has no sequence (`11 10 11 00`). Audio frames carry a positive sequence (`11 21 01 00`). The last packet sets the last-packet flag and a negative sequence (`11 23 01 00`). Server errors are message type `0xF` with a big-endian code and a UTF-8 message. Punctuation and ITN are turned on. `language` is omitted so the default model accepts mixed Chinese and English. `show_utterances` and `enable_nonstream` are on so a finished sentence is marked `definite`.

Handshake headers are `X-Api-App-Key` and `X-Api-Access-Key` for the legacy console, or `X-Api-Key` for the new console, plus `X-Api-Resource-Id`, `X-Api-Request-Id`, `X-Api-Connect-Id`, and `X-Api-Sequence: -1`. The default resource id is `volc.seedasr.sauc.duration` (model 2.0, duration billing). The 2.0 concurrent id and the 1.0 `volc.bigasr.sauc.duration` / `volc.bigasr.sauc.concurrent` ids are selectable.

`appKey`, `accessKey`, and `apiKey` are stored in Credential Manager under `tf_asr_volcano`, matching Type4Me's field names. `authMode` and `resourceId` stay in `tf_asr_volcano` in the JSON config, which is also where a Mac import lands. Each session opens a new socket and starts its sequence at 1.

Soniox is still built. Choosing it in Settings does not change hotkey, capture, or injection.

## Consequences

- A Volcengine account with streaming ASR enabled is required for the default path. There is no live call in CI.
- Frame encode/decode, gzip payloads, sequence and last-packet flags, utterance finals, and error codes are unit-tested without the network. An ignored test runs if `VOLC_APP_KEY` and `VOLC_ACCESS_KEY` are set.
- Elevated-window injection limits from ADR 0003 are unchanged.
