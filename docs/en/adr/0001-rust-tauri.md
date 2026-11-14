# ADR 0001: Rust and Tauri 2

[中文](../../zh/adr/0001-rust-tauri.md)

## Status

Accepted for the MVP.

## Context

Type4Me is a native Swift menu-bar app. The Windows port needs a tray-resident process, a global hotkey, WASAPI capture, a WebSocket client, and text injection. The cloud edition of the Mac app is roughly 10–20 MB. A Python sidecar or an Electron shell would miss that size and would add a runtime the hot path does not need.

The settings UI is small: a form for the hotkey, the Soniox key, and injection. It does not justify a heavy frontend stack.

## Decision

The core is Rust. The shell is Tauri 2 with a static HTML/CSS/JS settings page and a borderless overlay window. Tauri's WebView2 host is already on Windows 11, so the NSIS installer can stay a bootstrapper plus the Rust binary instead of embedding a browser.

Win32 (the low-level hook, `SendInput`, the clipboard, Credential Manager, the cursor position) is isolated in `src-tauri/src/platform` and compiled only for `cfg(windows)`. Pure logic lives in `type4we-core` and is tested without a Windows machine.

## Consequences

- The installer target is the same order of magnitude as the Mac cloud build, because WebView2 is a system component.
- CI must build on `windows-latest`. Linux can run the core tests and can type-check the Windows target, but it cannot execute WASAPI or the hook.
- A future local ASR engine is another Rust type behind `AsrEngine`. It does not require a new UI framework.
