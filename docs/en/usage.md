# Usage

[中文](../zh/usage.md)

## First run

Type4We starts in the notification area. Open **Settings** from the tray menu (left-click or right-click). The window's close button hides it; **Quit** exits the process.

1. Paste a Soniox API key and choose **Save key**. The box clears. The line under it says whether a key is stored. The key is not written to `config.json`.
2. Leave the model at `stt-rt-v6` unless you have a reason to pin `stt-rt-v5` (removed 15 December 2026).
3. Language hints default to `zh, en`. They bias recognition; they are not a hard language lock. Clear the field to send no hints.
4. Endpoint sensitivity defaults to `-0.3` (more patient, the Type4Me dictation preset). The Soniox range is `-1` to `1`. Higher values finalize sooner.
5. Choose push-to-talk or toggle, then **Save**.

## Dictating

Focus a normal text field. Hold **Right Alt** and speak. Release when you are done.

- Confirmed words are typed as Soniox marks them final, so text shows up while you are still talking.
- The overlay, when enabled, shows the full preview, including words that may still change. Those provisional words are typed once, at the end.
- The tray status moves through Starting, Listening, and Finishing.

Toggle mode: press the hotkey to start, press it again to stop. The key release does nothing.

Microphone privacy: if Windows has blocked desktop apps from the microphone, capture fails and the tray reports an error. Allow Type4We (or desktop apps) under Settings → Privacy → Microphone.

## Hotkey grammar

Tokens are separated by `+`. Modifiers: `Ctrl`, `Alt`, `Shift`, `Win`, plus `L`/`R` forms (`RAlt`, `LCtrl`, `RWin`). A trailing name can be a key (`Space`, `F8`, letters, digits). Examples that ship as presets: `RAlt`, `Ctrl+Win`, `RCtrl`.

`Ctrl+Win` is swallowed only once both keys are down, so a lone Windows key still opens Start. Holding Ctrl and Win and then Left still collides with the virtual-desktop shortcut, because the chord is already active before Left. Prefer Right Alt unless AltGr is in the way. [ADR 0004](adr/0004-default-hotkey.md).

## Injection

| Method | Behavior |
| --- | --- |
| Auto | `SendInput` unicode. Clipboard and Ctrl+V only if `SendInput` inserts fewer events than requested. |
| Unicode | Unicode only. A shortfall is shown as an error and the clipboard is left alone. |
| Clipboard | Always paste, then restore the previous Unicode text after a short pause. |

Elevated windows (anything you launched with "Run as administrator") drop this input. That is UIPI, not a failed keystroke the app can see. Run the target app as a normal user, or wait for a signed `uiAccess` build. [ADR 0003](adr/0003-text-injection.md).

Quick Mode is the only mode. The tray item is not a switch. Other mode names stored in the file are kept for a later import and are not executed.
