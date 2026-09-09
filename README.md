# Wave Player

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

### Track analysis

- Waveform generation
- BPM detection
- Musical key detection
- Audio metadata extraction
- Album and track information display
- Loudness analysis infrastructure

### Library and storage

- Recently played tracks
- Local favorites
- Persistent application settings
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

- [Rust](https://www.rust-lang.org/)
- [Dioxus](https://dioxuslabs.com/)
- [rodio](https://github.com/RustAudio/rodio)
- [Symphonia](https://github.com/pdeljanov/Symphonia)
- [SQLite](https://www.sqlite.org/)
- [rusqlite](https://github.com/rusqlite/rusqlite)
- [timestretch](https://crates.io/crates/timestretch)
- [math-audio-iir-fir](https://crates.io/crates/math-audio-iir-fir)
- [hound](https://crates.io/crates/hound)
- [Tailwind CSS](https://tailwindcss.com/)

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
│   ├── analysis/        # Waveform, BPM, key, and loudness analysis
│   ├── commands/        # Audio command handlers
│   ├── decoder.rs       # Audio decoding and pipeline construction
│   ├── filters.rs       # Live low-pass and high-pass filters
│   ├── offline_render.rs# Loop rendering and WAV export
│   ├── spatial.rs       # Width, pan, and Mid/Side processing
│   ├── state.rs         # Audio engine state
│   ├── store/            # SQLite, config, cache, favorites, and recents
│   └── timestretch.rs   # Real-time time stretching
├── gui/
│   ├── components/      # Reusable Dioxus components
│   ├── screens/         # Application screens
│   ├── layout.rs        # Main application layout
│   ├── route.rs         # Application routes
│   └── theme.rs         # Theme management
├── shared/
│   ├── command_types.rs # Commands sent to backend workers
│   ├── event_types.rs   # Events sent back to the UI
│   └── store_types.rs   # Shared data structures
└── main.rs              # Application bootstrap
```

## Requirements

Before running the project, install:

- Rust toolchain
- Cargo
- Node.js
- pnpm
- Dioxus CLI

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
dx serve --desktop
```

Or run both steps together:

```sh
pnpm build:css && dx serve --desktop
```

## Android development

Android support is experimental.

Build the stylesheet and start the Android target:

```sh
pnpm build:css && dx serve --android
```

Additional Android SDK and platform-specific setup may be required depending on the development environment.

## Building a release version

Build the Rust application in release mode:

```sh
cargo build --release
```

For the desktop application, use the Dioxus CLI according to the target platform:

```sh
dx build --release
```

The exact packaging process may differ between Windows, Linux, Android, and other supported targets.

## Data directories

The application currently stores runtime data in the `wave_player_data` directory.

This may include:

- SQLite database
- JSON configuration
- Cached waveform and analysis data

The desktop webview cache is stored separately in the `webview_cache` directory during development.

These paths are currently relative to the application working directory and may change in a future release.

## Current limitations

The project is still under active development. Known limitations include:

- Audio position is currently tracked by the playback engine clock and may not always represent sample-accurate output position.
- Switching pitch-preservation mode may rebuild the audio pipeline.
- Real-time and offline processing paths are still being aligned.
- The current processing model primarily targets mono and stereo audio.
- Background analysis results require additional protection against stale results after switching tracks.
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
- [ ] Crossfade between tracks
- [ ] Auto gain between tracks
- [ ] Better audio device recovery

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
- [ ] Improved real-time/offline processing parity
- [ ] Plugin hosting for VST3 and CLAP

### Analysis

- [x] Waveform visualization
- [x] BPM detection
- [x] Key detection
- [x] Metadata extraction
- [ ] Track structure detection
- [ ] Improved loudness workflow
- [ ] More reliable cancellation of outdated analysis jobs

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
