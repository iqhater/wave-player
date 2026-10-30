# player/library-metadata Specification

## Purpose
Описывает сохранение ручного BPM трека в библиотеке и правила приоритета между ручной правкой, результатом анализа и сбросом на детектированное значение.

## Requirements

### Requirement: Manual BPM is saved to the library

When the user commits a valid BPM for the current track, the system SHALL store the displayed rounded integer in `tracks.bpm` and keep the manual grid override. The value MUST survive track changes and application restart. Input outside `20..=300` or non-numeric MUST be rejected without changing the stored value.

**Verification:** unit test for `rounded_bpm` and commit validation; integration test for `update_track_bpm`; manual smoke for Enter, blur, restart, and invalid input.

#### Scenario: Committing a fractional value

- **WHEN** the user enters `127.4` and confirms with Enter
- **THEN** the player displays `127`, `tracks.bpm` contains `127.0`, and `ui.manual_grids` keeps the manual override for that track

#### Scenario: Committing by leaving the field

- **WHEN** the user edits the BPM and clicks elsewhere without pressing Enter
- **THEN** the edit is committed and saved exactly as if Enter had been pressed

#### Scenario: Value survives restart

- **WHEN** the user commits `128` and restarts the application
- **THEN** the player shows `128` for that track and the library still contains `128.0`

#### Scenario: Invalid input is rejected

- **WHEN** the user enters `10`, `400`, or `fast`
- **THEN** the stored BPM and the manual override are unchanged and the previous value is shown again

#### Scenario: Escape cancels the edit

- **WHEN** the user presses Escape while editing the BPM field
- **THEN** the draft is discarded and no database write occurs

#### Scenario: Track has no library row

- **WHEN** the current file has no row in `tracks` and the user commits a BPM
- **THEN** the player keeps working, the manual override is kept in config, and the failed database update is logged

### Requirement: Manual BPM overrides reanalysis

A manual BPM for a track MUST take precedence over the value produced by `Reanalyze`. Reanalysis MAY update the musical key, but MUST NOT overwrite `tracks.bpm` while a manual override exists for that track. The manual value remains until the user resets it.

**Verification:** unit test for `should_apply_detected_bpm`; integration test for store dispatch with a temporary config.

#### Scenario: Reanalysis keeps the manual value

- **WHEN** the track has manual BPM `128` and reanalysis detects `124`
- **THEN** `tracks.bpm` remains `128`, the player still shows `128`, and the key may be updated

#### Scenario: Manual override is per track

- **WHEN** one track has a manual BPM and another is reanalyzed
- **THEN** only the reanalyzed track's detected value is stored

#### Scenario: Reset allows the detected value

- **WHEN** the user resets the manual BPM and reanalysis completes
- **THEN** the detected BPM is stored and shown

### Requirement: Reset restores the detected BPM

Right-clicking the BPM field SHALL clear the manual override for the current track and restore the detected BPM in the player and in `tracks.bpm`. If no detected BPM is available, the player MUST show the unavailable state and MUST NOT write an unrelated value to the database.

**Verification:** unit test for the reset commit helper; integration test for the DB write/no-write cases.

#### Scenario: Reset with a detected value

- **WHEN** the manual value is `128`, the detected value is `124`, and the user right-clicks the BPM field
- **THEN** the player shows `124`, `tracks.bpm` becomes `124.0`, and the manual override is removed

#### Scenario: Reset without a detected value

- **WHEN** no detected BPM is available and the user resets the manual value
- **THEN** the player shows the unavailable state and `tracks.bpm` is left unchanged

### Requirement: BPM display and stored value use the same rounding

The BPM shown in the player and the rounded value stored in `tracks.bpm` MUST use the same rounding rule. The beat grid MAY keep the manually entered precision for alignment, but the library value MUST be the rounded integer shown to the user.

**Verification:** unit test for the shared rounding helper used by display and persistence.

#### Scenario: Rounding up

- **WHEN** the user enters `127.6`
- **THEN** the player and `tracks.bpm` both show `128`

#### Scenario: Grid keeps manual precision

- **WHEN** the user enters `127.4` and the beat grid uses the manual tempo
- **THEN** the grid alignment uses the entered `127.4`, while the displayed and stored library value is `127`
