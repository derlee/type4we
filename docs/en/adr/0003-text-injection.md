# ADR 0003: Text injection

[中文](../../zh/adr/0003-text-injection.md)

## Status

Accepted for the MVP.

## Context

The focused app should receive the transcript as if it had been typed. Writing straight to the clipboard and pasting disturbs whatever the user had copied. Some apps also handle `KEYEVENTF_UNICODE` poorly, so a paste path is still needed.

Windows User Interface Privilege Isolation (UIPI) blocks a medium-integrity process from sending input to a higher-integrity window. An app launched with "Run as administrator" is higher integrity. `SendInput` may report that the events were inserted even when the elevated window discards them. The supported fix is a signed binary with `uiAccess="true"` installed under a trusted path such as Program Files. Signing is out of scope for this MVP.

## Decision

The primary path plans one `KEYEVENTF_UNICODE` down/up pair per UTF-16 code unit, so non-BMP characters go out as surrogate pairs. A newline becomes an Enter key rather than a line-feed character.

`auto` (the default) calls `SendInput` first. If the returned count is lower than the number of events requested, Type4We falls back to the clipboard: it saves `CF_UNICODETEXT` when that format exists, sets the transcript, sends Ctrl+V, waits briefly, and writes the saved text back. `unicode` never touches the clipboard. `clipboard` always pastes.

Before injecting, the app waits up to 200 ms for the hotkey's modifiers to go up, so a held Ctrl does not turn the following keystrokes into shortcuts.

## Consequences

- Normal apps receive text without losing a text clipboard in the common case.
- The fallback restores Unicode text only. Files or images on the clipboard can be lost if the fallback runs.
- Elevated targets stay broken until phase 4 (signing and `uiAccess`). The app cannot reliably detect the silent drop, so the limitation is documented instead of papered over with a retry.
