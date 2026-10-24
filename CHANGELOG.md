## [unreleased]

### 🚀 Features

- *(audio)* Add grid sync and analysis settings

### 🐛 Bug Fixes

- *(ui)* Align grid and improve level meter

### ⚙️ Miscellaneous Tasks

- Publish cross-platform releases
## [0.1.0-alpha.14] - 2026-10-23

### 🚀 Features

- *(config)* Persist settings and window state

### 🐛 Bug Fixes

- Restart repeated tracks at source end
## [0.1.0-alpha.13] - 2026-10-16

### 🚀 Features

- Add cached album art to track lists
- *(player)* Add queue navigation and shuffle
- *(player)* Add realtime volume meter

### 🐛 Bug Fixes

- *(lufs)* Emit only final integrated reading
- Align track list columns
- Recheck source sample rate
- Prioritize loop over track repeat

### 🧪 Testing

- Align spatial tests with channel order
## [0.1.0-alpha.12] - 2026-09-13

### 🚀 Features

- *(ui)* Add App Info and close update notice

### 🐛 Bug Fixes

- *(ui)* Pin player controls across screens
- Restore playback cursor and loop labels
- Sync waveform zoom coordinates
## [0.1.0-alpha.11] - 2026-09-09

### 🚀 Features

- *(recent)* Add recent tracks list

### 🐛 Bug Fixes

- *(ui)* Align settings checkbox labels
- *(favorites)* Update track statistics live
- *(audio)* Preserve speed across mode switches
- Correct popup behavior and remove some bulk code
- *(ui)* Preserve player state

### 📚 Documentation

- Update README

### ⚙️ Miscellaneous Tasks

- *(release)* Bump to alpha.11
## [0.1.0-alpha.10] - 2026-09-04

### 🚀 Features

- *(ui)* Add responsive mobile controls
- *(ui)* Improve responsive audio upload
- *(mobile)* Improve Android file flow

### 🐛 Bug Fixes

- *(ui)* Adjust updates panel width
- Set desktop window favicon
- *(ui)* Fix mobile list and drop text alignment

### ⚙️ Miscellaneous Tasks

- *(release)* Bump version to 0.1.0-alpha.10
## [0.1.0-alpha.9] - 2026-08-29

### 🚀 Features

- Add favorites track management
- Improve favorites metadata and UI

### 🐛 Bug Fixes

- *(ui)* Refine player subtitle
## [0.1.0-alpha.8] - 2026-08-26

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
