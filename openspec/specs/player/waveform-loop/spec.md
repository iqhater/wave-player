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

### Requirement: Zoom and pan render from precomputed peaks

After peaks are received, zoom and pan MUST render from the precomputed peak pyramid without decoding audio and without requesting the backend on every viewport change. Frame render time MUST depend on the canvas width rather than on the track duration.

**Verification:** Node tests for the extracted JS cover level selection, aggregation, and the number of render operations; manual smoke on a one-hour mix.

#### Scenario: Rapid zoom near the end of a long track

- **WHEN** a one-hour track is loaded and the user rapidly zooms into a section in the last minutes
- **THEN** the waveform redraws from already loaded peaks without decoding and without new backend requests
- **AND** latency does not depend on whether the section is at the start or at the end of the track

#### Scenario: Zoom back and forth

- **WHEN** the user repeatedly zooms in and out on the same section
- **THEN** every zoom step redraws from memory without reloading data

#### Scenario: Canvas width changes

- **WHEN** the window or sidebar changes the canvas width
- **THEN** the waveform redraws from the same peaks and stays aligned in time
- **AND** no additional backend requests are sent

### Requirement: Progressive fallback prevents empty viewport regions

During zoom or pan, every visible pixel MUST render from the best available layer: detail, mip, or overview. When resolution is insufficient, a coarser level MUST be used per region; the canvas MUST NOT stay empty where data exists.

**Verification:** Node test for partial detail covers the filling of all columns; manual smoke for rapid Zoom Out.

#### Scenario: Fast zoom out after detail was loaded

- **WHEN** detail has been loaded for a zoomed-in section and the user rapidly zooms out
- **THEN** the areas to the left and right that are not covered by detail are filled from a coarser peak level
- **AND** no visible column stays empty

#### Scenario: Partial detail window coverage

- **WHEN** a local window covers only part of the current viewport
- **THEN** the covered part is drawn from detail and the rest from the mip level
- **AND** the boundary between layers does not create a gap or double rendering

#### Scenario: Pyramid is still loading

- **WHEN** the peak pyramid has not arrived yet but the overview is already available
- **THEN** the whole viewport is drawn from the overview until the pyramid arrives
- **AND** no empty areas appear

#### Scenario: Track has never been analyzed

- **WHEN** the track has neither an overview nor peaks yet
- **THEN** a neutral placeholder is displayed across the full canvas width
- **AND** the placeholder is replaced by the waveform when analysis completes

#### Scenario: Stale response after zoom out

- **WHEN** a response to an old detail request arrives after the viewport has changed
- **THEN** it does not replace the current rendering
- **AND** it may be stored in the window LRU for reuse

### Requirement: Multi-resolution peak pyramid is built once per track

During analysis, the finest min/max peak level MUST be built with a step of about 3 ms at 44.1/48 kHz and a bin count limit. Coarser levels MUST be built from it without re-decoding. Level selection MUST yield at most two bins per pixel.

**Verification:** unit tests for pyramid construction and level selection (Rust and Node); integration test for loading peaks from the cache.

#### Scenario: One-hour mix

- **WHEN** a one-hour track has been analyzed
- **THEN** the finest level contains min/max peaks with a step of about 128 frames and fits within the bin limit
- **AND** all coarser levels are built from it without accessing the audio file

#### Scenario: Short track

- **WHEN** a track several minutes long is loaded
- **THEN** the finest level step remains no coarser than about 3 ms
- **AND** the bin count and transfer size remain small

#### Scenario: Extremely long track

- **WHEN** the track duration exceeds the finest bin limit
- **THEN** the peak step increases to the next power of two
- **AND** analysis completes, and deep zoom is served by local windows

#### Scenario: Component remount

- **WHEN** the user leaves the screen and returns to the same track
- **THEN** the peak pyramid is requested again and restored without re-analyzing the file

### Requirement: Extreme zoom uses bounded local windows

At resolutions above the finest level, only a local window around the visible area (no more than a few seconds) MUST be decoded, and it MUST be cached. Repeated zoom or pan inside the window MUST NOT decode audio; alignment MUST be preserved.

**Verification:** unit/integration test of the window with exact seek and a marker; Node test for the LRU; manual smoke at maximum zoom.

#### Scenario: Zooming beyond the finest peak level

- **WHEN** the user zooms deeper than the finest level's resolution
- **THEN** only a small window around the visible area is decoded
- **AND** frames before the window start are not decoded on the main path

#### Scenario: Pan inside the loaded window

- **WHEN** the user pans inside an already loaded local window
- **THEN** no new request is sent and audio is not decoded again

#### Scenario: Returning to a previously viewed window

- **WHEN** the user leaves a local window and returns to it
- **THEN** previously loaded data is reused from the window cache

#### Scenario: Format without accurate seek

- **WHEN** the format does not support accurate seek
- **THEN** the local window is built correctly through the fallback sequential path
- **AND** the waveform position does not shift relative to the overview

### Requirement: Viewport synchronization does not block zoom rendering

The canvas MUST update immediately on every zoom or pan change. Synchronization of `zoom` and `view_center` with Dioxus MUST be rate-limited and MUST NOT trigger decoding, peak transfer, or component tree re-rendering on every wheel tick.

**Verification:** Node test confirms that the wheel path does not send load commands; manual smoke with rapid wheel input; code review of the synchronization paths.

#### Scenario: Rapid wheel input

- **WHEN** the user rapidly rotates the mouse wheel
- **THEN** the canvas follows the zoom on every animation frame
- **AND** viewport signals update less often than wheel events occur and do not start data loading

#### Scenario: Overlays catch up after the gesture

- **WHEN** the zoom change stops
- **THEN** loop overlays, beat grid, and the time scale reach the final viewport within about 100 ms
- **AND** the final position matches the waveform position

#### Scenario: Loop drag after zoom

- **WHEN** the user starts dragging the loop region after zooming
- **THEN** the drag uses the actual final viewport rather than an intermediate value

### Requirement: Peak rendering stays aligned with the timeline

The position of every bar MUST correspond to absolute track time regardless of the data source: mip level, local window, or overview. Switching between levels MUST NOT cause a horizontal jump of the waveform, cursor, or beat grid.

**Verification:** Node tests with known markers at different levels; unit test for a window that does not start at zero; manual smoke when crossing a resolution boundary.

#### Scenario: Known marker near the end

- **WHEN** there is a pronounced transient in the last seconds of the track and the user zooms into it
- **THEN** the transient is displayed at the same time position as on the overview
- **AND** the position does not shift when switching between peak levels

#### Scenario: Crossing the finest peak boundary

- **WHEN** the user smoothly increases zoom and switches from the mip level to the local window
- **THEN** the waveform does not jump horizontally and does not change scale
- **AND** the playback cursor stays at the same time

#### Scenario: Pan after a local window request

- **WHEN** the local window has been received and the user pans
- **THEN** new bars join the previous ones without a gap or double rendering

### Requirement: Waveform cache version 3 upgrades without losing user data

The waveform cache MUST store the overview and the finest peak level (version 3). Caches from the previous version MUST be ignored and recreated; a corrupted cache MUST NOT block analysis, playback, or the overview. `config.json`, SQLite, and user data MUST remain unchanged.

**Verification:** integration tests for round-trip, old version, corrupted file, and write failure; manual smoke after an update.

#### Scenario: Update with an old cache

- **WHEN** the application has been updated and a version 2 waveform cache remains on disk
- **THEN** the old file is ignored without an error
- **AND** the track is analyzed again and receives a version 3 cache

#### Scenario: Cache round-trip

- **WHEN** analysis completes and the waveform cache is saved
- **THEN** the overview and the peak pyramid load without losing display accuracy
- **AND** after a restart zoom works without a full analysis

#### Scenario: Corrupted cache

- **WHEN** the cache file is truncated or contains invalid values
- **THEN** the error is logged and the file is ignored
- **AND** the track is analyzed again, while playback and the interface are not blocked

#### Scenario: Cache cannot be written

- **WHEN** saving the waveform cache fails
- **THEN** analysis and playback continue
- **AND** the peak pyramid remains available in memory until the track changes or the application restarts
