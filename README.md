# Type4We

[中文说明](README.zh-CN.md)

Type4We is a Windows system-tray voice input app. Hold a hotkey, speak, and the transcript is typed at the cursor. This MVP is **Quick Mode** only: the raw transcript, with no LLM rewrite.

It is an independent Windows port of [Type4Me](https://github.com/joewongjc/type4me) by the Type4Me contributors. Type4Me is a macOS and iOS menu-bar app (hold a hotkey, speak, text lands at the cursor, with optional LLM polishing) and is licensed under the MIT License. Type4We does not copy Type4Me source. Shared configuration keys (`tf_selectedASRProvider`, `tf_asr_soniox`, `tf_language`, and the other `tf_*` groups) follow Type4Me so a later import of a Mac `credentials.json` is possible. See [NOTICE](#credit) below and the [license](LICENSE).

## What works in this MVP

- Tray icon with status, the current mode (Quick), Settings, and Quit.
- Global hotkey through a low-level keyboard hook (`WH_KEYBOARD_LL`), in push-to-talk (hold) or toggle. The default binding is **Right Alt**. Fn is not a usable default on Windows; see [ADR 0004](docs/en/adr/0004-default-hotkey.md).
- Microphone capture with WASAPI, downmixed to mono and resampled to 16 kHz PCM for the recognizer.
- **Volcengine** (豆包流式语音识别) is the default streaming provider, on `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async`. Final sentences are injected while you speak. A small overlay can show the live preview near the cursor.
- **Soniox** (`stt-rt-v6`) stays selectable in Settings. Both engines implement the same `AsrEngine` trait.
- Text injection with `SendInput` and `KEYEVENTF_UNICODE`. If that inserts nothing, Type4We pastes with the clipboard and then restores the previous Unicode clipboard text.
- Settings in `%APPDATA%\Type4We\config.json`. API keys go to Windows Credential Manager, not the JSON file.

Out of scope for now: LLM modes, history, Ask Anything, Windows actions, code signing, and auto-update. The [roadmap](#roadmap) lists those phases.

## Why Volcengine is the default

Derek asked for Volcengine (火山引擎 / Doubao streaming ASR) as the default cloud engine. The current bigmodel docs recommend the optimized bidirectional socket for live dictation, because it returns a frame when the transcript changes instead of once per audio packet. Type4We uses that endpoint, 200 ms PCM chunks, punctuation and ITN on, and no language lock so Chinese and English can mix. App ID and Access Token (or the new-console API key) are stored in Credential Manager. `resourceId` stays in `tf_asr_volcano`, the same key Type4Me uses. See [ADR 0005](docs/en/adr/0005-volcengine-default.md).

Docs checked 15 November 2026:

- [Bigmodel streaming WebSocket](https://www.volcengine.com/docs/6561/1354869)
- [Same protocol on the Doubao voice site](https://docs.volcengine.com/docs/doubao-voice/large-model-streaming-automatic-speech-recognition-api-1)

Soniox remains the alternative. Its JSON WebSocket is unchanged (`stt-rt-v6`; `stt-rt-v5` is accepted until 15 December 2026). [Soniox WebSocket API](https://docs.soniox.com/api-reference/stt/websocket-api).

## Build and run on Windows

Requirements: Rust stable, the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) (WebView2, which Windows 11 already has), and Node.js only if you want `cargo tauri dev`.

```powershell
git clone https://github.com/derlee/type4we.git
cd type4we
cargo test --workspace
cargo install tauri-cli --version "^2"
cargo tauri build --bundles nsis
```

The installer is `target\release\bundle\nsis\Type4We_0.1.0_x64-setup.exe` (the workspace target directory, not `src-tauri\target`). It installs for the current user and downloads the WebView2 bootstrapper only if WebView2 is missing. For a dev loop: `cargo tauri dev` (serves the static UI on port 1420).

Then:

1. Launch Type4We. It sits in the tray; the settings window stays hidden until you choose Settings.
2. In Settings, leave the provider on Volcengine. Create an app in the [Volcengine console](https://console.volcengine.com/) under Doubao Speech, enable streaming ASR 2.0 (resource `volc.seedasr.sauc.duration`), and save the App ID and Access Token. The token is write-only. Soniox is the other provider in the same menu.
3. Focus any normal (non-elevated) text field, hold Right Alt, and speak. Release to finish. Toggle mode starts and stops on each press.

Full steps: [setup](docs/en/setup.md) and [usage](docs/en/usage.md). Architecture: [overview](docs/en/architecture.md).

An example config with no secrets is [`config.example.json`](config.example.json).

## Known limitations

- **UIPI.** Windows does not deliver input from a medium-integrity process into an elevated window. `SendInput` can still report success. Typing into an app you started with "Run as administrator" needs a signed binary with `uiAccess`, which is phase 4. Details: [ADR 0003](docs/en/adr/0003-text-injection.md).
- **Right Alt is AltGr** on many non-US layouts. The hook swallows that key while it is the hotkey, so switch to `Ctrl+Win` or Right Ctrl in Settings. See [ADR 0004](docs/en/adr/0004-default-hotkey.md).
- The clipboard fallback restores **Unicode text**. If the fallback runs while the clipboard holds files or an image, that non-text data can be lost. Auto mode uses the clipboard only when Unicode insertion reports a shortfall.
- This tree was developed on Linux. WASAPI, the keyboard hook, `SendInput`, and Credential Manager were type-checked for `x86_64-pc-windows-msvc` and are built by GitHub Actions on `windows-latest`. They were not exercised on a physical Windows machine or against a live Volcengine or Soniox account in that environment.

## Roadmap

- **Phase 2.** LLM modes, prompts, history, and importing a Type4Me config (the `tf_*` shape is already reserved).
- **Phase 3.** Ask Anything through UI Automation, a whitelist of Windows actions, and more ASR providers (including Volcano Engine and a local engine behind the same `AsrEngine` trait).
- **Phase 4.** Code signing, `uiAccess` so elevated windows can receive input, a signed installer, and auto-update.

## Credit

Type4Me: <https://github.com/joewongjc/type4me> — Copyright (c) 2026 Type4Me Contributors, MIT License.

Type4We: Copyright (c) 2026 derlee and Type4We contributors, [MIT License](LICENSE).

## Docs

| English | 中文 |
| --- | --- |
| [Architecture](docs/en/architecture.md) | [架构](docs/zh/architecture.md) |
| [Setup](docs/en/setup.md) | [构建](docs/zh/setup.md) |
| [Usage](docs/en/usage.md) | [使用](docs/zh/usage.md) |
| [ADR 0001 Rust + Tauri 2](docs/en/adr/0001-rust-tauri.md) | [ADR 0001](docs/zh/adr/0001-rust-tauri.md) |
| [ADR 0002 Cloud ASR](docs/en/adr/0002-cloud-asr.md) | [ADR 0002](docs/zh/adr/0002-cloud-asr.md) |
| [ADR 0005 Volcengine default](docs/en/adr/0005-volcengine-default.md) | [ADR 0005](docs/zh/adr/0005-volcengine-default.md) |
| [ADR 0003 Text injection](docs/en/adr/0003-text-injection.md) | [ADR 0003](docs/zh/adr/0003-text-injection.md) |
| [ADR 0004 Default hotkey](docs/en/adr/0004-default-hotkey.md) | [ADR 0004](docs/zh/adr/0004-default-hotkey.md) |
