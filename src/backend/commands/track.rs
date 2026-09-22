/*
- LoadFile
*/
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rodio::Source;

use super::playback;
use crate::backend::analysis::bpm_key::analyze_bpm_key;
use crate::backend::analysis::lufs::start_loudness_analysis;
use crate::backend::analysis::waveform::stream_waveform;
use crate::backend::cover_cache;
use crate::backend::decoder::{build_audio_pipeline, load_samples_mono, open_file_with_decoder};
use crate::backend::metadata::parse_track_metadata_with_cover;

use crate::backend::state::AudioState;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{AudioError, PlayerEvent};
use crate::shared::store_types::TrackData;

/// Loads a track, builds the audio pipeline and starts playback.
pub(crate) fn load_file(state: &mut AudioState, path: PathBuf) -> Result<(), AudioError> {
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

    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();

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

    start_waveform_analysis(
        path.clone(),
        channels.get(),
        sample_rate.get(),
        duration,
        state,
    );

    start_loudness_analysis(path.clone(), &state.status_tx);

    start_bpm_analysis(
        path,
        duration,
        state.status_tx.clone(),
        state.store_cmd_tx.clone(),
    );

    Ok(())
}

fn start_waveform_analysis(
    path: PathBuf,
    channels: u16,
    sample_rate: u32,
    duration: Duration,
    state: &AudioState,
) {
    let status_tx = state.status_tx.clone();

    let num_bins = 500;

    stream_waveform(
        path,
        channels,
        sample_rate,
        duration,
        num_bins,
        move |event| {
            status_tx
                .try_broadcast(event)
                .map(|_| ())
                .map_err(|error| error.into())
        },
    );
}

fn start_bpm_analysis(
    path: PathBuf,
    duration: Duration,
    status_tx: crate::shared::event_types::StatusSender,
    store_cmd_tx: flume::Sender<StoreCommand>,
) {
    std::thread::spawn(move || {
        let analysis_limit = duration
            .as_secs()
            .saturating_add(u64::from(duration.subsec_nanos() > 0))
            .min(u32::MAX as u64) as u32;

        let Ok((samples, sample_rate, _duration)) = load_samples_mono(&path, analysis_limit.max(1))
        else {
            return;
        };

        let Ok(result) = analyze_bpm_key(&samples, sample_rate, 1) else {
            return;
        };

        let _ = store_cmd_tx.send(StoreCommand::UpdateTrackAnalysis {
            path: path.clone(),
            bpm: result.bpm,
            key: result.key_name.clone(),
        });

        let _ = status_tx.try_broadcast(PlayerEvent::BpmKeyDetected {
            bpm: result.bpm,
            beat_grid: result.beat_grid,
            key_name: result.key_name,
            key_numerical: result.key_numerical,
        });
    });
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

    let decoder = open_file_with_decoder(&path)?;

    let channels = decoder.channels().get();
    let sample_rate = decoder.sample_rate().get();

    let total_frames = state
        .track
        .duration
        .as_secs_f64()
        .mul_add(sample_rate as f64, 0.0) as usize;

    let start_frame = (start.as_secs_f64() * sample_rate as f64) as usize;
    let end_frame = (end.as_secs_f64() * sample_rate as f64) as usize;

    drop(decoder);

    let status_tx = state.status_tx.clone();

    crate::backend::analysis::waveform::stream_waveform_detail(
        path,
        channels,
        sample_rate,
        total_frames,
        start_frame,
        end_frame,
        points,
        request_id,
        move |event| {
            status_tx
                .try_broadcast(event)
                .map(|_| ())
                .map_err(|error| error.into())
        },
    );

    Ok(())
}

pub(crate) fn request_waveform_overview(state: &AudioState) -> Result<(), AudioError> {
    let Some(path) = state.track.path.clone() else {
        return Ok(());
    };

    let decoder = open_file_with_decoder(&path)?;

    let channels = decoder.channels().get();
    let sample_rate = decoder.sample_rate().get();
    let duration = decoder.total_duration().unwrap_or(state.track.duration);

    drop(decoder);

    start_waveform_analysis(path, channels, sample_rate, duration, state);

    Ok(())
}
