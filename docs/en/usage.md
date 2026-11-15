# Usage

[中文](../zh/usage.md)

## First run

Type4We starts in the notification area. Open **Settings** from the tray menu (left-click or right-click). The window's close button hides it; **Quit** exits the process.

1. Leave **Provider** on Volcengine. Under Doubao Speech in the [Volcengine console](https://console.volcengine.com/), create an application and enable streaming speech recognition. The default resource is `volc.seedasr.sauc.duration` (model 2.0, billed by audio duration). The other menu values are the 2.0 concurrent plan and the 1.0 `volc.bigasr.sauc.*` plans.
2. For the legacy console, paste the App ID and Access Token and choose **Save credentials**. The token is not written back to the form. For the new console, switch Authentication to API Key and save that key instead. Neither value is stored in `config.json`.
3. To use Soniox instead, change Provider, save a key from [console.soniox.com](https://console.soniox.com), and leave the model at `stt-rt-v6` (`stt-rt-v5` is removed 15 December 2026). Language hints default to `zh, en`. Endpoint sensitivity defaults to `-0.3`.
4. Choose push-to-talk or toggle, then **Save**.

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
