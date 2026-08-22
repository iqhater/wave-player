/*
- LoadFile
*/
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rodio::Source;

use crate::backend::analysis::bpm_key::analyze_bpm_key;
use crate::backend::analysis::waveform::stream_waveform;
use crate::backend::decoder::{build_audio_pipeline, load_samples_mono, open_file_with_decoder};
use crate::backend::metadata::parse_track_metadata;
use crate::backend::state::AudioState;
use crate::shared::event_types::{AudioError, PlayerEvent};

/// Loads a track, builds the audio pipeline and starts playback.
pub(crate) fn load_file(state: &mut AudioState, path: PathBuf) -> Result<(), AudioError> {
    state.player.stop();
    state.player.clear();

    state.track.is_loaded = false;
    state.track.is_playing = false;
    state.track.position = Duration::ZERO;
    state.track.started_at = None;

    let path_ref = Path::new(&path);

    let decoder = open_file_with_decoder(path_ref)?;

    let duration = decoder.total_duration().unwrap_or(Duration::ZERO);

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
    // state.player.set_speed(state.playback.speed);
    state.player.play();

    let metadata = parse_track_metadata(path_ref);

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

    start_bpm_analysis(path, state.status_tx.clone());

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

fn start_bpm_analysis(path: PathBuf, status_tx: crate::shared::event_types::StatusSender) {
    std::thread::spawn(move || {
        let Ok((samples, sample_rate, _duration)) = load_samples_mono(&path, 90) else {
            return;
        };

        let Ok(result) = analyze_bpm_key(&samples, sample_rate, 1) else {
            return;
        };

        let _ = status_tx.try_broadcast(PlayerEvent::BpmKeyDetected {
            bpm: result.bpm,
            beat_offset: result.beat_offset,
            key_name: result.key_name,
            key_numerical: result.key_numerical,
        });
    });
}
