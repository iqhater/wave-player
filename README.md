# Wave Player

[![License: MIT](https://img.shields.io/badge/license-MIT-yellow.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Dioxus 0.7](https://img.shields.io/badge/Dioxus-0.7-blue.svg)](https://dioxuslabs.com/)

> [!WARNING]
> **Unstable alpha software.** This project is in early development. Expect
> crashes, audio glitches, incomplete features, breaking changes, and
> unoptimized CPU/RAM usage. First-time analysis and export can be slow on
> large files. Configuration and database formats may change without notice.
>
> Do not rely on Wave Player as the only player for important sessions, and
> back up the `wave_player_data` directory before testing a new build.

Wave Player is a desktop-focused music player and audio playground built with Rust and Dioxus.

The project combines a responsive interface with real-time audio processing features such as looping, time stretching, filters, stereo processing, waveform visualization, track analysis, and offline WAV export.

> The project is currently in an early alpha stage. APIs, audio behavior, and UI structure may change.

## Features

### Playback

- Play, pause, resume, and seek audio tracks
- Adjustable playback speed
- Optional pitch preservation when changing playback speed
- Volume control
- Whole-track repeat
- Loop region playback
- Waveform-based timeline navigation
- Responsive desktop and mobile-oriented layouts

### Audio processing

- Live low-pass filter
- Live high-pass filter
- Stereo width control
- Stereo panning
- Mid/Side monitoring modes:
  - Stereo
  - Mid
  - Side
- Smooth transitions for filter and spatial mode changes to reduce clicks and artifacts
- Real-time time stretching with optional pitch preservation
- Cancellable, latest-request-wins background analysis
- Cached waveform, BPM, key, beat grid, and loudness results

### Track analysis

- Waveform generation
- BPM detection
- Musical key detection
- Audio metadata extraction
- Album and track information display
- Integrated LUFS loudness analysis

### Library and storage

- Recently played tracks
- Local favorites
- Persistent application settings, including window size, position, and maximized state
- Atomic `config.json` writes
- SQLite-backed track data
- Database migrations
- Cached analysis data

### Export

- Export the selected loop region to WAV
- Apply playback speed, filters, and spatial processing during export
- Export progress reporting

### Interface

- Dark and light themes
- Responsive layout
- Desktop and mobile-oriented UI structure
- Custom bundled fonts and icons
- Dioxus-based component architecture

## Supported audio formats

Audio decoding is provided through `rodio` and Symphonia.

The project is intended to support common formats including:

- MP3
- WAV
- OGG/Vorbis
- FLAC
- AAC
- AIFF

Actual format support may depend on the decoder capabilities and the specific file encoding.

## Technology stack

- [Rust](https://www.rust-lang.org/) 1.85+ (edition 2024)
- [Dioxus](https://dioxuslabs.com/) 0.7
- [rodio](https://github.com/RustAudio/rodio) 0.22
- [Symphonia](https://github.com/pdeljanov/Symphonia)
- [stratum-dsp](https://crates.io/crates/stratum-dsp)
- [ebur128](https://crates.io/crates/ebur128)
- [timestretch](https://crates.io/crates/timestretch)
- [math-iir-fir](https://crates.io/crates/math-iir-fir)
- [hound](https://crates.io/crates/hound)
- [rusqlite](https://github.com/rusqlite/rusqlite) and SQLite
- [audiotags](https://crates.io/crates/audiotags)
- [flume](https://crates.io/crates/flume)
- [async-broadcast](https://crates.io/crates/async-broadcast)
- [Tailwind CSS](https://tailwindcss.com/) 4

## Architecture

Wave Player uses a flat modular architecture that is intentionally kept simple for the current project size.

```text
┌──────────────┐
│   Dioxus UI  │
└──────┬───────┘
       │ PlayerCommand
       ▼
┌────────────────────┐
│   Audio Engine     │
│                    │
│  Playback state    │
│  Decoder           │
│  Time stretching   │
│  Filters           │
│  Spatial processing│
└──────┬─────────────┘
       │ PlayerEvent
       ▼
┌──────────────┐
│   Dioxus UI  │
└──────────────┘
```

Persistent storage runs independently from the audio engine:

```text
┌──────────────┐
│   Dioxus UI  │
└──────┬───────┘
       │ StoreCommand
       ▼
┌────────────────────┐
│    Store Thread    │
│                    │
│  SQLite database   │
│  JSON configuration│
│  Cache management  │
└──────┬─────────────┘
       │ StoreEvent
       ▼
┌──────────────┐
│   Dioxus UI  │
└──────────────┘
```

### Main source directories

```text
src/
├── backend/
│   ├── analysis/         # Waveform, BPM, key, LUFS, scheduling, and cache
│   ├── commands/         # Audio command handlers
│   ├── store/            # SQLite, config, cache, favorites, and recents
│   ├── decoder.rs        # Audio decoding and pipeline construction
│   ├── engine_v2.rs      # Audio engine event loop
│   ├── filters.rs        # Live low-pass and high-pass filters
│   ├── offline_render.rs # Loop rendering and WAV export
│   ├── spatial.rs        # Width, pan, and Mid/Side processing
│   ├── state.rs          # Audio engine state
│   ├── timestretch.rs    # Real-time time stretching
│   └── waveform_cache.rs # Versioned waveform file cache
├── gui/
│   ├── components/       # Reusable Dioxus components
│   ├── screens/          # Application screens
│   ├── layout.rs         # Main application layout
│   ├── route.rs          # Application routes
│   └── theme.rs          # Theme management
├── shared/
│   ├── command_types.rs  # Commands sent to backend workers
│   ├── event_types.rs    # Events sent back to the UI
│   └── store_types.rs    # Shared data structures
└── main.rs               # Application bootstrap
```

## Requirements

Before running the project, install:

- Rust toolchain
- Cargo
- Node.js
- pnpm
- Dioxus CLI
- Rust 1.85 or newer (the project uses edition 2024)
- A system webview runtime: WebView2 on Windows, WebKitGTK on Linux, or the system WKWebView on macOS
- Recent Node.js LTS and pnpm

The build reads `assets/tailwind.css` at compile time, so generate it before
running `cargo build` or `dx build`:

```sh
pnpm install
pnpm build:css
```

Install the Dioxus CLI if necessary:

```sh
cargo install dioxus-cli
```

Check the installation:

```sh
rustc --version
cargo --version
node --version
pnpm --version
dx --version
```

## Running the project

Install frontend dependencies:

```sh
pnpm install
```

Build the Tailwind stylesheet:

```sh
pnpm build:css
```

Run the desktop application:

```sh
dx serve --platform desktop
```

Or run both steps together:

```sh
pnpm build:css && dx serve --desktop
```

## Android development

Android support is experimental.

Build the stylesheet and start the Android target:

```sh
pnpm build:css && dx serve --platform android
```

Additional Android SDK and platform-specific setup may be required depending on the development environment.

## Building a release version

Build the Rust application in release mode:

```sh
cargo build --release
```

For the desktop application, use the Dioxus CLI according to the target platform:

```sh
pnpm build:css
dx bundle --release
```
The packaging output and available bundle formats depend on the target
platform. Check the Dioxus CLI output for the exact location.

The exact packaging process may differ between Windows, Linux, Android, and other supported targets.

## Data directories

The application currently stores runtime data in the `wave_player_data` directory.

This may include:

- SQLite database
- JSON configuration
- Cached waveform and analysis data

The `config.json` file stores theme, audio processing settings, UI toggles,
and the last window size, position, and maximized state.

The data directory is resolved relative to the current working directory.
Launch the application from the same directory to keep settings, history,
and cached analysis data.

The desktop webview cache is stored separately in the `webview_cache` directory during development.

These paths are currently relative to the application working directory and may change in a future release.

## Current limitations

The project is still under active development. Known limitations include:

- Audio position is currently tracked by the playback engine clock and may not always represent sample-accurate output position.
- Switching pitch-preservation mode may rebuild the audio pipeline.
- Real-time and offline processing paths are still being aligned.
- The current processing model primarily targets mono and stereo audio.
- First-time analysis of long tracks can still consume noticeable CPU time; results are cached afterwards.
- There is no automatic recovery from corrupted cache or database files yet.
- Windows is the primary development and testing platform; Linux, macOS, and Android builds are experimental.
- Audio device recovery and device hot-swapping are not fully implemented.
- Plugin hosting is not implemented yet.
- There is no full playlist management system yet.
- Crossfading between tracks is not implemented yet.

## Roadmap

### Playback and library

- [x] Basic playback controls
- [x] Seek through the waveform
- [x] Track repeat
- [x] Loop region playback
- [x] Recently played tracks
- [x] Local favorites
- [ ] Playlist management
- [ ] Auto gain between tracks
- [x] Persistent window geometry

### Audio processing

- [x] Playback speed control
- [x] Optional pitch preservation
- [x] Low-pass filter
- [x] High-pass filter
- [x] Stereo width
- [x] Stereo panning
- [x] Mid/Side monitoring
- [ ] Parametric EQ
- [ ] Compressor
- [ ] Limiter
- [ ] Plugin hosting for VST3 and CLAP

### Analysis

- [x] Waveform visualization
- [x] BPM detection
- [x] Key detection
- [x] Metadata extraction
- [x] Cancellation of outdated analysis jobs
- [x] Persistent waveform and analysis cache

### User interface

- [x] Responsive layout
- [x] Dark theme
- [x] Light theme
- [x] Custom fonts and icons
- [ ] More complete accessibility support
- [ ] Keyboard shortcut system
- [ ] Improved mobile interaction model

## Development principles

The project follows several important design principles:

- Keep the audio engine independent from the UI.
- Avoid blocking the real-time audio path.
- Communicate with the audio engine through explicit commands.
- Keep DSP effects composable.
- Prefer explicit state transitions over implicit global state.
- Validate parameters before they reach the audio pipeline.
- Keep real-time and offline processing behavior consistent.
- Use Rust ownership and types to make invalid audio states harder to represent.

## Development commands

Format the code:

```sh
cargo fmt
```

Check the project:

```sh
cargo check --all-targets --all-features
```

Run Clippy:

```sh
cargo clippy --all-targets --all-features -- -D warnings
```

Run tests:

```sh
cargo test --all-features
```

Build the CSS:

```sh
pnpm build:css
```

## Reporting issues

Wave Player is alpha software, so bug reports are especially useful. When
opening an issue, include:

- operating system and version;
- Wave Player version or commit hash;
- steps to reproduce the problem;
- audio format and a short non-copyrighted test file if possible;
- console output or the exact error message;
- whether the issue reproduces with a clean `wave_player_data` directory.

Please do not upload copyrighted music. A short synthetic test file is enough.

## Contributing

Bug reports and pull requests are welcome. For significant changes, open an
issue first so the approach can be discussed before implementation.

## License

Wave Player is distributed under the MIT License.

See [LICENSE](LICENSE) for details.

## Project status

Wave Player is a personal project created to build a convenient and modern audio player for everyday use.

The idea came from my own experience as both a music producer and a regular listener. Existing music players often provide only basic playback controls, while useful information such as waveform visualization, BPM, musical key, loudness, metadata, and loop-based listening is either unavailable or separated across different tools.

Wave Player aims to bring these features together in one focused application for listening to, exploring, and analyzing music.

The main goals are:

- Build a convenient audio player for personal everyday use.
- Create a modern tool for both music production and casual listening.
- Make additional track analysis available directly during playback.
- Combine waveform visualization, BPM, key, metadata, and audio controls in one interface.
- Explore real-time audio processing and DSP with Rust.
- Develop a maintainable foundation for future audio tools.
- Experiment with desktop and mobile application development using Dioxus.
