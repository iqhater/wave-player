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
use crate::backend::decoder::{build_audio_pipeline, load_samples_mono, open_file_with_decoder};
use crate::backend::metadata::parse_track_metadata;
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
    println!("Duration {:?}", duration);

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

    let metadata = parse_track_metadata(path_ref);

    let track_data = TrackData {
        title: metadata.title.clone(),
        artist: metadata.artist.clone(),
        album: (!metadata.album.is_empty()).then(|| metadata.album.clone()),
        genre: (!metadata.genre.is_empty()).then(|| metadata.genre.clone()),
        duration: duration.as_millis() as i64,
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

    start_bpm_analysis(path, state.status_tx.clone(), state.store_cmd_tx.clone());

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
    status_tx: crate::shared::event_types::StatusSender,
    store_cmd_tx: flume::Sender<StoreCommand>,
) {
    std::thread::spawn(move || {
        let Ok((samples, sample_rate, _duration)) = load_samples_mono(&path, 90) else {
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
            beat_offset: result.beat_offset,
            key_name: result.key_name,
            key_numerical: result.key_numerical,
        });
    });
}
