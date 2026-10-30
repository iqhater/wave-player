# player/waveform-loop Specification

## Purpose
Описывает поведение перетаскивания и изменения размера loop-региона на waveform: отзывчивость во время анализа трека, единую координатную модель, границы региона и безопасное завершение drag.

## Requirements

### Requirement: Loop region remains responsive during track analysis

While a newly loaded track is being analyzed, dragging or resizing the loop region MUST remain responsive: the region MUST follow the pointer without perceptible stalls or a sticky cursor. Analysis progress MUST NOT block pointer handling or leave the region lagging behind the pointer after release.

**Verification:** manual smoke during the full analysis of a long track; unit tests cover the pure drag geometry. OS scheduling and WebView timing are not deterministic in unit tests.

#### Scenario: Drag starts during analysis

- **WHEN** the user loads a track, analysis is still running, and the user drags the loop region
- **THEN** the region follows the pointer while analysis continues
- **AND** it stops at the pointer-up position without continuing to move afterwards

#### Scenario: Analysis completes during the drag

- **WHEN** analysis finishes while the user is dragging
- **THEN** the drag continues normally and the loop values do not jump

#### Scenario: Repeated drags after analysis

- **WHEN** the user drags the region again after analysis has finished
- **THEN** the region remains responsive

### Requirement: Playback cursor keeps moving during loop drag

While the user drags or resizes the loop region, the playback cursor MUST continue to reflect the current playback position. Suspending heavy waveform redraws during the drag MUST NOT freeze the cursor.

**Verification:** manual smoke during playback; the cursor must advance while peaks/detail redraws are suspended.

#### Scenario: Playing track during a drag

- **WHEN** a track is playing and the user drags the loop region
- **THEN** the playback cursor keeps advancing at the playback rate
- **AND** waveform peaks and detail may update again when the drag ends

### Requirement: Loop drag uses a single coordinate model

The rendered position of the loop region and the drag math MUST use the same mapping between track time and viewport position, including after zoom and pan. The mapping MUST be derived from the current zoom and view center; stale pixel offsets measured before a layout change MUST NOT be used.

**Verification:** unit test for `time_to_viewport_pct` at multiple zoom/center values; manual smoke after zoom/pan and resize.

#### Scenario: Drag at default zoom

- **WHEN** the user drags the middle of the region at zoom `1`
- **THEN** the region follows the pointer without offset

#### Scenario: Drag after zoom and pan

- **WHEN** the user zooms and pans the waveform and then drags the loop region
- **THEN** the marker under the pointer follows the pointer exactly and the region does not drift

#### Scenario: Layout changed between drags

- **WHEN** the window is resized or the sidebar is collapsed between two drags
- **THEN** the next drag uses the new container geometry and has no offset

#### Scenario: Hover preview disabled

- **WHEN** the hover preview cursor is disabled in Settings
- **THEN** dragging and resizing the loop region work exactly as before

#### Scenario: Volume meter is visible

- **WHEN** the volume meter is visible next to the waveform
- **THEN** the loop region and markers stay within the waveform canvas rather than the meter area
- **AND** the right marker at track end aligns with the waveform's right edge

### Requirement: Loop bounds, minimum length, and drag completion

Loop bounds MUST stay within `[0, duration]`, start MUST remain at least `0.5` s before end, middle drag MUST preserve length, and a shorter new track MUST clamp the loop. `SetLoop` MUST be sent only when the drag ends; pointer-up or pointer-cancel outside the waveform MUST end the drag without leaving a blocking overlay.

**Verification:** unit test for the drag/clamp/cancel state machine; manual smoke for pointer-up outside the window and Escape.

#### Scenario: Left marker crosses the right marker

- **WHEN** the user drags the left marker past the right marker
- **THEN** the left marker stops `0.5` seconds before the right marker

#### Scenario: Middle drag reaches a track boundary

- **WHEN** the user drags the middle of the region beyond the track start or end
- **THEN** the region is clamped to the boundary and its length is preserved

#### Scenario: Pointer-up commits once

- **WHEN** the user releases the pointer after a drag
- **THEN** exactly one `SetLoop` command is sent with the final start and end values

#### Scenario: Drag does not seek

- **WHEN** the user finishes dragging the loop region
- **THEN** the playback position does not jump to the pointer-up position

#### Scenario: Drag is cancelled

- **WHEN** the user presses Escape or the pointer is cancelled during a drag
- **THEN** the region returns to the values from the drag start and no `SetLoop` command is sent

#### Scenario: Released outside the waveform

- **WHEN** the user releases the pointer outside the waveform or outside the window
- **THEN** the drag ends, the overlay and pointer capture are released, and the next drag works without an extra click

#### Scenario: New track is shorter than the loop

- **WHEN** a new track is shorter than the current loop end
- **THEN** the loop is clamped into the new duration

### Requirement: Analysis runs at background priority without disturbing loop or controls

Waveform, BPM, key, and loudness analysis SHALL run on worker threads at idle or below-normal OS priority. Waveform results MUST update only the waveform canvas; BPM, key, and loudness results MUST update only their dedicated displays. Analysis results MUST NOT rebuild loop or player-control state and MUST NOT block the audio engine thread.

**Verification:** manual smoke during full analysis plus code review; OS scheduling cannot be asserted deterministically in unit tests.

#### Scenario: Analysis runs while dragging

- **WHEN** analysis is running and the user drags the loop region
- **THEN** pointer handling stays responsive and the loop start and end values are unaffected by analysis results

#### Scenario: Analysis completes

- **WHEN** analysis finishes
- **THEN** the waveform and BPM/key displays update, the loop region does not jump, and the player controls keep their state

#### Scenario: Priority change is rejected

- **WHEN** the OS rejects the requested worker priority
- **THEN** analysis still runs, the failure is logged, and the application remains usable

#### Scenario: Track change resets peaks once

- **WHEN** a new track starts
- **THEN** the waveform peaks are reset once and the new analysis data replaces them without replaying the previous track's peaks
