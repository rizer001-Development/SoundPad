# Soundpad

**Free, open-source soundpad for gamers and streamers**

![Development status](https://img.shields.io/badge/status-Stable-green)

Soundpad is a lightweight, cross-platform soundboard application designed for ease of use. Play sounds instantly with hotkeys, organize your sounds into categories, and route audio to any application.

---

### Organization Docs

[![Guide](https://img.shields.io/badge/Guide-rizer001--Development-00AEFF)](https://github.com/rizer001-Development/.github/blob/main/GUIDE.md) · [![Contributing](https://img.shields.io/badge/Contributing-rizer001--Development-4CAF50)](https://github.com/rizer001-Development/.github/blob/main/CONTRIBUTING.md) · [![Security](https://img.shields.io/badge/Security-rizer001--Development-D9534F)](https://github.com/rizer001-Development/.github/blob/main/SECURITY.md) · [![Code of Conduct](https://img.shields.io/badge/Code%20of%20Conduct-rizer001--Development-5BC0DE)](https://github.com/rizer001-Development/.github/blob/main/CODE_OF_CONDUCT.md)

## Features

- **Multi-format support** — MP3, WAV, OGG, FLAC, M4A, AAC (via symphonia)
- **Global hotkeys** — Play sounds even when the app is minimized. Bound keys are *passive*: they are observed, not consumed, so a game still receives the key (bind W and you walk forward *and* play the sound)
- **Categories** — Organize sounds into groups (Memes, Alerts, Music, etc.)
- **Presets** — Save and load sound collections as JSON files
- **Master & per-sound volume** — Control overall and individual output volume
- **Output device selection** — Route audio to a virtual cable (VB-Cable, VoiceMeeter, …) for Discord, OBS, Teams
- **Looping** — Toggle per-sound loop playback
- **Dark/Light theme** — Choose your preferred look
- **Search** — Instantly find sounds by name
- **Drag & drop** — The whole window is a drop target for audio files: drag them onto any part of the app (including the title bar / empty space) and a pulsing "⬇ Drop to add sounds" overlay shows where to drop them
- **Lightweight** — Native binary, no JVM, fast startup, low memory

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) 1.85+ (edition 2024)
- Linux only: ALSA dev packages (`libasound2-dev`)

### Build & Run

```bash
cargo run --release
```

The release binary is written to `target/release/soundpad` (or `soundpad.exe` on Windows). Launch helpers are available:

- Windows: `scripts\Launch.bat`
- Linux/macOS: `./scripts/Soundpad.sh`

### Run tests

```bash
cargo test
```

## Usage

1. **Add sounds** — drag & drop audio files into the window or use **＋ Add sounds**.
   Files are copied into the app's portable `sounds/` folder, so the library survives moving the original files.
2. **Assign hotkeys** — open a sound's edit dialog (pencil icon) and set a hotkey, e.g. `F5` or `Ctrl+Shift+1`.
   Global hotkeys work even when the window is not focused (toggle them in Settings).
3. **Organize** — create categories in the sidebar and drag sounds between them via the edit dialog.
4. **Presets** — save your current library as a named preset and restore it later.
5. **Settings** — master volume, output device, theme, and hotkey on/off.

## Data location

All data lives next to the binary (portable mode — when run via `cargo run`, in the project root) as plain JSON:

```
data/sounds.json       — sound library
data/categories.json   — categories
data/settings.json     — app settings
presets/*.json         — saved presets
sounds/                — copies of the audio files
```

## Project structure

```
src/
├── main.rs     — egui UI (sidebar, grid, dialogs, status bar)
├── models.rs   — data types (SoundFile, Category, Preset, Settings)
├── store.rs    — JSON persistence
├── audio.rs    — multi-voice playback engine (rodio + cpal)
└── hotkeys.rs  — global hotkeys (passive keyboard polling via device_query / GetAsyncKeyState)
```

## Tech stack

| Rust (this repo) |
|---|
| Compose Desktop | egui / eframe |
| javax.sound + FFmpeg | rodio + symphonia |
| JNativeHook | device_query (passive key polling) |
| kotlinx.serialization | serde_json |
| SQLite (DatabaseManager) | JSON files (portable, no native deps) |

## Contributing

Contributions are welcome. Please see the organization [Contributing guide](https://github.com/rizer001-Development/.github/blob/main/CONTRIBUTING.md).

## License

This project is licensed under the **GNU Affero General Public License v3.0** — see the [LICENSE](LICENSE) file for details.
