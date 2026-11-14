# ADR 0004: Default hotkey

[中文](../../zh/adr/0004-default-hotkey.md)

## Status

Accepted. The default binding is Right Alt (`RAlt`), push-to-talk.

## Context

Type4Me's gesture is "hold Fn". On macOS, Fn is visible to `CGEvent`. On Windows it generally is not. Most laptop firmware handles Fn before the key ever becomes a virtual-key code, and the few machines that emit a scan code do not agree on which one. `WH_KEYBOARD_LL` therefore cannot offer Fn as a portable default.

`RegisterHotKey` is the wrong API for hold-to-talk. It fires when the chord is first pressed and does not give a reliable key-up for a modifier-only chord. The MVP installs `WH_KEYBOARD_LL` on its own thread with a message pump, and matches edges in pure code so the same rules are unit-tested.

## Decision

The default is **Right Alt**, held for push-to-talk.

- It is one key, so the gesture matches "hold Fn" more closely than a chord.
- The hook sees `VK_RMENU` on normal hardware, including the left/right distinction, so Left Alt and Alt+Tab keep working.
- The hook swallows Right Alt while it is the binding, so the focused app does not open its menu.

**Ctrl+Win** is a built-in preset, not the default. Both keys are visible to the hook, and the hook swallows the Windows key only after Ctrl is already down, so a lone Win key still opens Start. The cost is collisions: Ctrl+Win+Left/Right switches virtual desktops, and the chord becomes active before the arrow key, so dictation would start as well. Right Ctrl is the other preset for people who want a single key that is not Alt.

## Consequences

- On layouts where Right Alt is AltGr, holding it to dictate is fine, but typing AltGr characters is not, because the key is swallowed. Those users should switch to `Ctrl+Win` or `RCtrl` in Settings. The binding is a string in `config.json` and takes effect without a restart.
- Fn is not offered. A future build could add a per-machine scan code, but it would not be the default.
