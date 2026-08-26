## [unreleased]

### 🚀 Features

- Add offline audio rendering
- Integrate audio rendering commands
- Update playback controls

### 🐛 Bug Fixes

- *(ui)* Close popups on outside click
- *(ui)* Color knob ticks on drag
- *(theme)* Swap dark mode hover shadows

### 💼 Other

- Skip production release builds

### 🎨 Styling

- Update player styles

### ⚙️ Miscellaneous Tasks

- Update audio dependencies
- *(release)* Bump version to alpha 8
## [0.1.0-alpha.7] - 2026-08-22

### 🚀 Features

- Add 3.5 dB decoder gain

### 🐛 Bug Fixes

- Correct waveform binning and peak detection

### 🚜 Refactor

- Complete player state integration
## [0.1.0-alpha.6] - 2026-08-14

### 🚀 Features

- Add drag-and-drop file loading to the player
- *(player)* Improve waveform and audio controls

### 🐛 Bug Fixes

- Center empty-text checkboxes under filter knobs
- Keep timeline cursor label visible at track edges
- Reset timeline cursor when loading a new track
- Set sane default loop region on track load
## [0.1.0-alpha.5] - 2026-08-08

### 🚀 Features

- Add dark theme support
- *(version)* Read app version from Cargo.toml

### 🐛 Bug Fixes

- Fix volume panel shift and loop drag cursor
- *(filters)* Rework LP/HP filters with math-iir-fir
## [0.1.0-alpha.4] - 2026-08-07

### 🚀 Features

- Highlight loop toggle when enabled

### 🐛 Bug Fixes

- Fix waveform zoom anchoring at edges and center
- Fix waveform seek position on click
- Add background borders to ui top loop markers
- Keep loop handle drag active outside the region

### ⚙️ Miscellaneous Tasks

- Add changelog for v0.1.0-alpha.1
## [0.1.0-alpha.1] - 2026-08-06

### 🚀 Features

- Initial release of wave player
