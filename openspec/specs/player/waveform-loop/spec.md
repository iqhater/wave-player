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

### Requirement: Time scale matches the waveform canvas width

The time scale under the waveform MUST occupy exactly the same horizontal area as the waveform canvas. `VolumeMeter` and the gap beside it MUST NOT increase the scale width, and hiding the meter MUST NOT reserve space in the scale.

**Verification:** manual smoke with the meter shown/hidden, after resizing the window and collapsing the sidebar; WebView geometry is not reproducible in unit tests.

#### Scenario: Meter is visible

- **WHEN** a track is loaded, the window is wider than the `sm` breakpoint, and the time scale is enabled
- **THEN** the right edge of the scale matches the right edge of the waveform canvas
- **AND** the `07:00` mark for a 7:09 track is displayed inside the waveform, not past its end

#### Scenario: Meter is hidden

- **WHEN** the window is narrower than the `sm` breakpoint and `VolumeMeter` is hidden
- **THEN** the scale still matches the canvas width
- **AND** no empty gap is left on the right for the meter

#### Scenario: Canvas width changes

- **WHEN** the user resizes the window or collapses the sidebar
- **THEN** the scale automatically takes the new canvas width without manual recomputation and without desynchronizing from the waveform

### Requirement: Time marks adapt to the available width

The number of time marks MUST depend on the waveform canvas width: as the width decreases, the step MUST increase so that adjacent labels do not overlap. The step MUST remain a "round" interval, and mark density MUST be preserved on wide windows.

**Verification:** `time_marks` unit tests at different canvas widths and manual smoke while narrowing the window.

#### Scenario: Narrow window

- **WHEN** the window is narrowed and `MM:SS` labels no longer fit at the current step
- **THEN** the number of marks decreases by increasing the step
- **AND** the minimum gap between adjacent marks is at least 40 px

#### Scenario: Wide window

- **WHEN** the canvas is wide enough for 15 marks
- **THEN** mark density and step remain unchanged

### Requirement: Time marks stay within the visible range

The position of every mark MUST be computed in the same coordinate system as the waveform and the playback cursor. At any zoom and pan, a mark percentage MUST stay within `[0, 100]` of the canvas width, and marks MUST cover only the visible part of the track.

**Verification:** unit tests of the pure mark calculation function and manual smoke during zoom/pan.

#### Scenario: Full track at zoom 1

- **WHEN** a track is loaded and zoom is `1`
- **THEN** marks cover the whole visible range, the first mark is at `0%`, and every percentage is within `[0, 100]`

#### Scenario: Zoom and pan

- **WHEN** the user zooms and scrolls the waveform
- **THEN** the scale shows marks only for the visible range
- **AND** no mark appears before `0%` or after `100%`

#### Scenario: Scale disabled or no track

- **WHEN** time marks are disabled in Settings or no track is loaded
- **THEN** the scale is not displayed and the rest of the waveform layout does not shift
