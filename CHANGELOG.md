# Changelog

All notable changes to SoundPad are documented here.  
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).  
This project adheres to [Semantic Versioning](https://semver.org/).

## [1.1.0] — 2026-09-12

### Added

- **Full-window drag and drop.** The whole window is now a drop target for audio files (not just a specific widget). Dragging files over the app shows a pulsing "⬇ Drop to add sounds" overlay until you drop them.
- **Animated UI pass.** Hover lift, animated accent border and glow, press feedback, custom play button with hover/press scaling and a pulsing halo while playing, animated 4-bar equalizer during playback, smooth progress bar, animated sidebar rows, and a pulsing drop overlay. ~60 fps repaint keeps it fluid.
- **Passive global hotkeys.** Replaced hook-based hotkey registration with keyboard polling (Windows `GetAsyncKeyState` via `device_query`). Bound keys are observed, not consumed — a game still receives the key (e.g. bind W and you walk forward *and* play the sound).

### Fixed

- **Hotkey "Bind" dialog closing too early.** The "Press any key combination..." capture prompt now stays visible until a chord is actually captured (or Esc / Cancel / uncheck is used). The capture state no longer resets every frame.
- **Hotkey binding swallowing keys.** Bound keys no longer block the key from reaching other apps. No more "W plays the sound but the game doesn't move."

### Changed

- Tech stack: `global-hotkey` crate replaced by `device_query`.
- README: updated feature list and tech-stack table to reflect the Rust rewrite and the passive hotkey behavior.

### Notes

- Portable mode: all data (sounds, categories, settings, presets) lives next to the binary in the `data/` and `sounds/` folders.
- Requires Windows (current build); Linux build is supported in `cargo` but no Linux release binary is shipped in this tag yet.
