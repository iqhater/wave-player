/*
- Play
- Pause
- Resume
- Seek
- SetSpeed
- SetVolume
- SetPreservePitch
- ToggleRepeatTrack
*/

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::backend::decoder::{build_audio_pipeline, open_file_with_decoder};
use crate::backend::state::{AudioState, LoopState};
use crate::shared::event_types::{AudioError, PlayerEvent};
use std::sync::atomic::Ordering;

use super::track;

/// Starts playback for a file.
///
/// File loading remains implemented in the track command module.
pub(crate) fn play(state: &mut AudioState, path: PathBuf) -> Result<(), AudioError> {
    state.queue.set_single(path.clone());
    state.publish_queue_state();

    track::load_file(state, path)
}

pub(crate) fn play_queue(
    state: &mut AudioState,
    tracks: Vec<PathBuf>,
    current: PathBuf,
) -> Result<(), AudioError> {
    if tracks.is_empty() {
        return Ok(());
    }

    let same_track = state.track.path.as_ref() == Some(&current);

    state.queue.replace(tracks, current);
    state.publish_queue_state();

    if same_track {
        if is_playing(state) {
            pause(state)?;
        } else {
            resume(state)?;
        }

        return Ok(());
    }

    let Some(path) = state.queue.current_path().cloned() else {
        return Ok(());
    };

    track::load_file(state, path)
}

pub(crate) fn set_shuffle(state: &mut AudioState, enabled: bool) -> Result<(), AudioError> {
    state.queue.set_shuffle(enabled);
    state.publish_queue_state();

    Ok(())
}

pub(crate) fn next_track(state: &mut AudioState) -> Result<(), AudioError> {
    let Some(path) = state.queue.peek_next().cloned() else {
        return Ok(());
    };

    track::load_file(state, path)?;
    state.queue.advance_next();
    state.publish_queue_state();

    Ok(())
}

const PREVIOUS_RESTART_THRESHOLD: Duration = Duration::from_secs(3);

pub(crate) fn previous_track(state: &mut AudioState) -> Result<(), AudioError> {
    if !state.track.is_loaded {
        return Ok(());
    }

    update_position(state);

    if state.track.position > PREVIOUS_RESTART_THRESHOLD {
        return seek(state, Duration::ZERO);
    }

    let Some(path) = state.queue.peek_previous().cloned() else {
        return seek(state, Duration::ZERO);
    };

    track::load_file(state, path)?;
    state.queue.advance_previous();
    state.publish_queue_state();

    Ok(())
}

/// Pauses the current track.
pub(crate) fn pause(state: &mut AudioState) -> Result<(), AudioError> {
    if !state.track.is_loaded || state.player.is_paused() {
        return Ok(());
    }

    update_position(state);

    state.player.pause();
    state.track.is_playing = false;
    state.track.started_at = None;

    let _ = state.status_tx.try_broadcast(PlayerEvent::Paused);

    Ok(())
}

/// Resumes the current track.
pub(crate) fn resume(state: &mut AudioState) -> Result<(), AudioError> {
    if !state.track.is_loaded || !state.player.is_paused() {
        return Ok(());
    }

    state.player.play();
    state.track.is_playing = true;
    state.track.started_at = Some(Instant::now());

    let _ = state.status_tx.try_broadcast(PlayerEvent::Resumed);

    Ok(())
}

/// Seeks to a position in the original track timeline.
pub(crate) fn seek(state: &mut AudioState, position: Duration) -> Result<(), AudioError> {
    if !state.track.is_loaded {
        return Ok(());
    }

    let position = position.min(state.track.duration);

    if state.playback.preserve_pitch {
        return seek_with_wsola(state, position);
    }

    state
        .player
        .try_seek(position)
        .map_err(|_| AudioError::SeekFailed)?;

    state.track.position = position;
    state.track.started_at = state.track.is_playing.then(Instant::now);

    state
        .playback
        .playback_progress
        .store(position.as_millis() as u32, Ordering::Relaxed);

    Ok(())
}

/// Changes playback speed.
pub(crate) fn set_speed(state: &mut AudioState, speed: f32) -> Result<(), AudioError> {
    let speed = speed.clamp(0.25, 2.0);

    if state.track.is_playing {
        update_position(state);
    }

    state.playback.speed = speed;
    apply_playback_speed(state);

    Ok(())
}

/// Changes output volume.
pub(crate) fn set_volume(state: &mut AudioState, volume: f32) -> Result<(), AudioError> {
    let volume = volume.clamp(0.0, 1.0);

    state.playback.volume = volume;
    state.player.set_volume(volume);

    /* let _ = state.status_tx.try_broadcast(PlayerEvent::VolumeChanged {
        volume: state.playback.volume,
    }); */

    Ok(())
}

/// Sets whether to preserve pitch when changing speed.
pub(crate) fn set_preserve_pitch(state: &mut AudioState, enabled: bool) -> Result<(), AudioError> {
    if state.playback.preserve_pitch == enabled {
        return Ok(());
    }

    if state.track.is_playing {
        update_position(state);
    }

    // let previous_value = state.playback.preserve_pitch;
    state.playback.preserve_pitch = enabled;
    state
        .playback
        .preserve_pitch_shared
        .store(enabled, Ordering::Release);

    if state.track.is_loaded {
        if let Err(error) = reload_track_with_effects(state) {
            // state.playback.preserve_pitch = previous_value;
            return Err(error);
        }
    }

    Ok(())
}

/// Enables or disables whole-track repeat.
pub(crate) fn toggle_repeat(state: &mut AudioState, enabled: bool) -> Result<(), AudioError> {
    state.track.repeat = enabled;

    Ok(())
}

/// Play count criteria in percents
const PLAY_COUNT_THRESHOLD: f32 = 0.60;

fn advance_position(state: &mut AudioState) -> bool {
    let Some(started_at) = state.track.started_at else {
        return false;
    };

    let now = Instant::now();
    let elapsed = now.duration_since(started_at);

    // Учитываем именно реальное время.
    // Скорость воспроизведения здесь намеренно не используется.
    state.track.session_listened_time += elapsed;

    state.track.position = state
        .track
        .position
        .saturating_add(elapsed.mul_f32(state.playback.speed))
        .min(state.track.duration);

    state.track.started_at = Some(now);

    record_play_count_if_needed(state);

    true
}

fn record_play_count_if_needed(state: &mut AudioState) {
    if state.track.play_count_recorded || state.track.duration.is_zero() {
        return;
    }

    let threshold = state.track.duration.mul_f32(PLAY_COUNT_THRESHOLD);

    if state.track.session_listened_time < threshold {
        return;
    }

    let Some(path) = state.track.path.clone() else {
        return;
    };

    state.track.play_count_recorded = true;

    let _ = state
        .store_cmd_tx
        .send(crate::shared::command_types::StoreCommand::SaveTrackPlaysCount { path });
}

/// Updates the tracked position of the current track.
fn update_position(state: &mut AudioState) {
    if !advance_position(state) {
        return;
    }
    state.set_progress(state.track.position);
}

pub(crate) fn apply_playback_speed(state: &AudioState) {
    state
        .playback
        .speed_shared
        .store(state.playback.speed.to_bits(), Ordering::Release);

    // Player не должен дополнительно менять скорость.
    state.player.set_speed(1.0);
}

fn reload_track_with_effects(state: &mut AudioState) -> Result<(), AudioError> {
    let position = state.track.position.min(state.track.duration);

    reload_track_with_effects_at(state, position)
}

fn reload_track_with_effects_at(
    state: &mut AudioState,
    position: Duration,
) -> Result<(), AudioError> {
    let Some(path) = state.track.path.clone() else {
        return Ok(());
    };

    let position = position.min(state.track.duration);
    let was_playing = state.track.is_playing && !state.player.is_paused();

    let decoder = open_file_with_decoder(&path)?;

    state.player.stop();
    state.player.clear();

    // Position must be applied to the decoder before adding the source.
    build_audio_pipeline(state, decoder, position)?;

    state.player.set_volume(state.playback.volume);

    apply_playback_speed(state);

    if was_playing {
        state.player.play();

        state.track.is_playing = true;
        state.track.started_at = Some(Instant::now());
    } else {
        state.player.pause();

        state.track.is_playing = false;
        state.track.started_at = None;
    }

    state.track.position = position;

    state
        .playback
        .playback_progress
        .store(position.as_millis() as u32, Ordering::Relaxed);

    Ok(())
}

fn seek_with_wsola(state: &mut AudioState, position: Duration) -> Result<(), AudioError> {
    let Some(path) = state.track.path.clone() else {
        return Err(AudioError::FileCorrupted);
    };

    let was_playing = state.track.is_playing && !state.player.is_paused();

    let decoder = open_file_with_decoder(&path)?;

    state.player.stop();
    state.player.clear();

    build_audio_pipeline(state, decoder, position)?;

    state.player.set_volume(state.playback.volume);

    if was_playing {
        state.player.play();
        state.track.is_playing = true;
        state.track.started_at = Some(Instant::now());
    } else {
        state.player.pause();
        state.track.is_playing = false;
        state.track.started_at = None;
    }

    state.track.position = position;

    state
        .playback
        .playback_progress
        .store(position.as_millis() as u32, Ordering::Relaxed);

    Ok(())
}

pub(crate) fn is_playing(state: &AudioState) -> bool {
    state.track.is_loaded && state.track.is_playing && !state.player.is_paused()
}

pub(crate) fn tick(state: &mut AudioState) -> Result<(), AudioError> {
    if state.track.is_loaded && !state.track.repeat && state.player.empty() {
        if loop_boundary_reached(
            &state.loop_state,
            state.track.position,
            state.track.duration,
            true,
        ) {
            seek_to_loop_start(state)?;
            return Ok(());
        }

        if state.track.position < state.track.duration {
            state.track.position = state.track.duration;
            finish_track(state)?;
            return Ok(());
        }
    }

    if !state.track.is_loaded || !state.track.is_playing {
        return Ok(());
    }

    if !advance_position(state) {
        return Ok(());
    }

    if loop_boundary_reached(
        &state.loop_state,
        state.track.position,
        state.track.duration,
        state.player.empty(),
    ) {
        seek_to_loop_start(state)?;
        return Ok(());
    }

    if repeat_early(state) {
        repeat_from_beginning(state)?;
        return Ok(());
    }

    if state.track.position >= state.track.duration {
        finish_track(state)?;
        return Ok(());
    }

    state.publish_progress_if_needed();

    Ok(())
}

fn loop_is_active(loop_state: &LoopState) -> bool {
    loop_state.enabled && loop_state.end > loop_state.start
}

const REPEAT_GUARD: Duration = Duration::from_millis(200);

fn repeat_early(state: &AudioState) -> bool {
    state.track.repeat
        && !loop_is_active(&state.loop_state)
        && state.track.duration > REPEAT_GUARD
        && state.track.position >= state.track.duration.saturating_sub(REPEAT_GUARD)
}

fn loop_boundary_reached(
    loop_state: &LoopState,
    position: Duration,
    track_duration: Duration,
    source_empty: bool,
) -> bool {
    if !loop_is_active(loop_state) {
        return false;
    }

    position >= loop_state.end
        // При loop.end == track_duration родительский источник может
        // завершиться раньше, чем позиция успеет достичь duration.
        || (source_empty && loop_state.end == track_duration)
}

fn seek_to_loop_start(state: &mut AudioState) -> Result<(), AudioError> {
    let loop_start = state.loop_state.start;

    if state.player.empty() {
        reload_track_with_effects_at(state, loop_start)
    } else {
        seek(state, loop_start)
    }
}

fn finish_track(state: &mut AudioState) -> Result<(), AudioError> {
    if state.track.repeat {
        return repeat_from_beginning(state);
    }

    if load_next_from_queue(state)? {
        return Ok(());
    }

    mark_finished(state)
}

fn mark_finished(state: &mut AudioState) -> Result<(), AudioError> {
    state
        .player
        .try_seek(Duration::ZERO)
        .map_err(|_| AudioError::SeekFailed)?;

    state.player.pause();

    state.track.is_playing = false;
    state.track.started_at = None;
    state.track.position = Duration::ZERO;

    state.set_progress(Duration::ZERO);

    let _ = state.status_tx.try_broadcast(PlayerEvent::Finished);

    Ok(())
}

fn repeat_from_beginning(state: &mut AudioState) -> Result<(), AudioError> {
    if state.player.empty() {
        return mark_finished(state);
    }

    state.player.pause();

    state
        .player
        .try_seek(Duration::ZERO)
        .map_err(|_| AudioError::SeekFailed)?;

    state.track.position = Duration::ZERO;
    state.track.is_playing = true;
    state.track.started_at = Some(Instant::now());
    state.track.session_listened_time = Duration::ZERO;
    state.track.play_count_recorded = false;

    state.set_progress(Duration::ZERO);

    state.player.play();

    Ok(())
}

fn load_next_from_queue(state: &mut AudioState) -> Result<bool, AudioError> {
    let Some(path) = state.queue.peek_next().cloned() else {
        return Ok(false);
    };

    track::load_file(state, path)?;
    state.queue.advance_next();
    state.publish_queue_state();

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::loop_boundary_reached;
    use crate::backend::state::LoopState;
    use std::time::Duration;

    #[test]
    fn loop_at_track_end_restarts_when_source_is_empty() {
        let loop_state = LoopState {
            enabled: true,
            start: Duration::from_secs(5),
            end: Duration::from_secs(10),
        };

        assert!(loop_boundary_reached(
            &loop_state,
            Duration::from_secs(9),
            Duration::from_secs(10),
            true,
        ));
    }

    #[test]
    fn loop_before_track_end_waits_until_loop_boundary() {
        let loop_state = LoopState {
            enabled: true,
            start: Duration::from_secs(5),
            end: Duration::from_secs(8),
        };

        assert!(!loop_boundary_reached(
            &loop_state,
            Duration::from_secs(7),
            Duration::from_secs(10),
            true,
        ));
    }

    #[test]
    fn disabled_loop_does_not_restart() {
        let loop_state = LoopState {
            enabled: false,
            start: Duration::from_secs(5),
            end: Duration::from_secs(10),
        };

        assert!(!loop_boundary_reached(
            &loop_state,
            Duration::from_secs(10),
            Duration::from_secs(10),
            true,
        ));
    }

    #[test]
    fn zero_length_loop_does_not_restart() {
        let loop_state = LoopState {
            enabled: true,
            start: Duration::from_secs(5),
            end: Duration::from_secs(5),
        };

        assert!(!loop_boundary_reached(
            &loop_state,
            Duration::from_secs(5),
            Duration::from_secs(10),
            false,
        ));
    }
}

#[test]
fn loop_boundary_has_priority_over_track_repeat() {
    let loop_state = LoopState {
        enabled: true,
        start: Duration::from_secs(5),
        end: Duration::from_secs(10),
    };

    assert!(loop_boundary_reached(
        &loop_state,
        Duration::from_secs(10),
        Duration::from_secs(10),
        false,
    ));
}
