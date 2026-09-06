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
use crate::backend::state::AudioState;
use crate::shared::event_types::{AudioError, PlayerEvent};
use std::sync::atomic::Ordering;

use super::track;

/// Starts playback for a file.
///
/// File loading remains implemented in the track command module.
pub(crate) fn play(state: &mut AudioState, path: PathBuf) -> Result<(), AudioError> {
    // replace into init or dispatch???
    track::load_file(state, path)
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

    // Мгновенно передаем флаг режима в аудио-поток.
    // На следующем блоке источник сам пересоберет внутренний движок!
    /* state
    .playback
    .preserve_pitch_shared
    .store(enabled, Ordering::Relaxed); */

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
    let Some(path) = state.track.path.clone() else {
        return Ok(());
    };

    let position = state.track.position.min(state.track.duration);
    let was_playing = state.track.is_playing && !state.player.is_paused();

    /* let source_position = if state.playback.preserve_pitch {
        position
    } else {
        position.div_f32(state.playback.speed.max(0.01))
    }; */
    let source_position = position;

    let decoder = open_file_with_decoder(&path)?;

    state.player.stop();
    state.player.clear();

    // Позиция устанавливается внутри Decoder до добавления источника.
    build_audio_pipeline(state, decoder, source_position)?;

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

    /* let player_speed = if state.playback.preserve_pitch {
        1.0
    } else {
        state.playback.speed
    };

    state.player.set_speed(player_speed); */

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
    if state.track.is_loaded
        && state.player.empty()
        && state.track.position < state.track.duration
        && !state.track.repeat
    {
        state.track.position = state.track.duration;
        finish_track(state)?;
        return Ok(());
    }

    if !state.track.is_loaded || !state.track.is_playing {
        return Ok(());
    }

    if !advance_position(state) {
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

    if jump_to_loop_start(state) {
        seek(state, state.loop_state.start)?;
        return Ok(());
    }

    state.publish_progress_if_needed();

    Ok(())
}

const REPEAT_GUARD: Duration = Duration::from_millis(200);

fn repeat_early(state: &AudioState) -> bool {
    state.track.repeat
        && state.track.duration > REPEAT_GUARD
        && state.track.position >= state.track.duration.saturating_sub(REPEAT_GUARD)
}

fn jump_to_loop_start(state: &AudioState) -> bool {
    state.loop_state.enabled
        && state.loop_state.end > state.loop_state.start
        && state.track.position >= state.loop_state.end
}

fn finish_track(state: &mut AudioState) -> Result<(), AudioError> {
    if state.track.repeat {
        repeat_from_beginning(state)
    } else {
        mark_finished(state)
    }
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
