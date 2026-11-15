# Setup and build

[中文](../zh/setup.md)

## Prerequisites

- Windows 10 or 11, 64-bit.
- [Rust](https://rustup.rs/) stable (1.90 or newer; Tauri 2.12 asks for 1.90).
- [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/). It is already installed on current Windows 11. The NSIS installer uses the download bootstrapper when it is missing, so the installer stays small.
- Node.js is only required for `cargo tauri dev`, which serves the static `ui/` folder. `cargo tauri build` copies `ui/` as-is and does not need Node.

Visual Studio Build Tools with the MSVC toolchain are required by Rust on Windows. `rustup` will point at them.

## Build

```powershell
cargo test --workspace
cargo install tauri-cli --version "^2"
cargo tauri build --bundles nsis
```

Outputs:

- `target\release\type4we.exe`
- `target\release\bundle\nsis\Type4We_0.1.0_x64-setup.exe`

Cargo puts these in the workspace `target` directory, not under `src-tauri`.

The installer is per-user (`currentUser`), so it does not ask for an administrator. That also means the process runs at medium integrity and cannot type into elevated windows. See [ADR 0003](adr/0003-text-injection.md).

Development:

```powershell
cargo tauri dev
```

That runs `node scripts/dev-server.cjs`, which serves `ui/` at `http://127.0.0.1:1420`.

## Continuous integration

`.github/workflows/ci.yml` runs on `windows-latest`. It executes `cargo test --workspace --locked`, builds the NSIS bundle with `tauri-apps/tauri-action`, and uploads the installer and `type4we.exe` as the `type4we-windows` artifact.

The core library tests do not need Windows. They cover the session machine, config load/save (including stripping API keys), hotkey chords, resampling, and Soniox message fixtures taken from the public docs.

## Configuration file

The app reads `%APPDATA%\Type4We\config.json`. A missing file is created from the defaults the first time you save. [`config.example.json`](../../config.example.json) is the same shape with no secrets.

API keys are not part of that file. If a copied Type4Me `credentials.json` still contains `apiKey` or `accessKey` under a `tf_asr_*` or `tf_llm_*` object, the first load moves those strings into Credential Manager and rewrites the JSON without them. The credential target is `com.type4we.grouped:tf_asr_soniox:apiKey`.

Logs append to `%APPDATA%\Type4We\logs\type4we.log`.

## Volcengine credentials

1. Sign in at <https://console.volcengine.com/>.
2. Open Doubao Speech (豆包语音) and enable streaming speech recognition. Model 2.0 is the current recommendation; model 1.0 still works.
3. Create an application. The legacy console shows an App ID and an Access Token. The new console shows an API Key. Type4We accepts either: leave Authentication on "App ID + Access Token", or switch it to "API Key".
4. Pick a resource id. The default, `volc.seedasr.sauc.duration`, is model 2.0 billed by audio duration. `volc.seedasr.sauc.concurrent` is the concurrent plan. `volc.bigasr.sauc.duration` and `volc.bigasr.sauc.concurrent` are the model 1.0 equivalents. A mismatch here is reported by the service as not granted, not as a bad token.
5. Save the credentials in Type4We Settings. The token or API key goes to Windows Credential Manager. `resourceId` and `authMode` are written to `%APPDATA%\Type4We\config.json` under `tf_asr_volcano`.

To use Soniox instead, change the provider in that same screen and save a key from <https://console.soniox.com>.
