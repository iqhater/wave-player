/*
- LoadFile
*/
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rodio::Source;

use super::playback;
use crate::backend::cover_cache;
use crate::backend::decoder::{build_audio_pipeline, open_file_with_decoder};
use crate::backend::metadata::parse_track_metadata_with_cover;

use crate::backend::state::AudioState;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{AudioError, PlayerEvent};
use crate::shared::store_types::TrackData;

/// Loads a track, builds the audio pipeline and starts playback.
pub(crate) fn load_file(state: &mut AudioState, path: PathBuf) -> Result<(), AudioError> {
    state.analysis.cancel();
    state.track.path = None;
    state.track.duration = Duration::ZERO;

    state.player.stop();
    state.player.clear();

    // speed sync
    playback::apply_playback_speed(state);

    state.track.is_loaded = false;
    state.track.is_playing = false;
    state.track.position = Duration::ZERO;
    state.track.started_at = None;
    state.track.session_listened_time = Duration::ZERO;
    state.track.play_count_recorded = false;

    let path_ref = Path::new(&path);

    let decoder = open_file_with_decoder(path_ref)?;

    let duration = decoder.total_duration().unwrap_or(Duration::ZERO);
    // println!("Duration {:?}", duration);

    build_audio_pipeline(state, decoder, Duration::ZERO)?;

    state.track.path = Some(path.clone());
    state.track.duration = duration;
    state.track.position = Duration::ZERO;
    state.track.is_loaded = true;
    state.track.is_playing = true;
    state.track.started_at = Some(Instant::now());

    state.playback.playback_progress.store(0, Ordering::Relaxed);

    state.player.set_volume(state.playback.volume);
    crate::backend::commands::playback::apply_playback_speed(state);

    state.player.play();

    let cached_cover = cover_cache::load_cover(&state.cover_cache_dir, path_ref);

    let metadata = parse_track_metadata_with_cover(path_ref, cached_cover);

    let track_data = TrackData {
        title: metadata.title.clone(),
        artist: metadata.artist.clone(),
        album: (!metadata.album.is_empty()).then(|| metadata.album.clone()),
        genre: (!metadata.genre.is_empty()).then(|| metadata.genre.clone()),
        duration: duration.as_millis() as i64,
        cover: metadata.cover.clone(),
    };

    let _ = state.store_cmd_tx.send(StoreCommand::RecordRecentTrack {
        path: path.clone(),
        track_data,
    });

    let _ = state
        .status_tx
        .try_broadcast(PlayerEvent::Started { duration, metadata });

    state.analysis.start(path, duration);

    Ok(())
}

pub(crate) fn request_waveform_detail(
    state: &AudioState,
    request_id: u64,
    start: Duration,
    end: Duration,
    points: usize,
) -> Result<(), AudioError> {
    let Some(path) = state.track.path.clone() else {
        return Ok(());
    };

    state
        .analysis
        .request_detail(path, state.track.duration, request_id, start, end, points);

    Ok(())
}

pub(crate) fn request_waveform_overview(state: &AudioState) -> Result<(), AudioError> {
    state.analysis.replay_overview(&state.status_tx);

    Ok(())
}
