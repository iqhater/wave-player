use super::filters::LiveFilteredSource;
use super::spatial::LiveSpatialSource;
use super::state::AudioState;
use crate::shared::event_types::AudioError;
use rodio::Source;
use std::fs::File;
use std::io::BufReader;
use std::num::NonZeroU16;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use crate::backend::timestretch::TimeStretchSource;
use rodio::{Decoder, Player, decoder::DecoderBuilder};
use rodio_wsola::WsolaSourceExt;

use std::fs;

// ---------------------------------------------------------------------------
// Decoding helpers
// ---------------------------------------------------------------------------

/// Открывает файл, создаёт декодер, добавляет его в плеер.
/// Возвращает длительность трека в случае успеха.
pub fn load_file_with_decoder(
    path: &Path,
    player: &Player,
    lp_cutoff: Arc<AtomicU32>,
    hp_cutoff: Arc<AtomicU32>,
    ms_mode: Arc<AtomicU32>,
    width: Arc<AtomicU32>,
    pan: Arc<AtomicU32>,
) -> Result<(Duration, NonZeroU16, NonZeroU32), AudioError> {
    let file = File::open(path)
        .map_err(|_| AudioError::FileNotFound(path.to_string_lossy().into_owned()))?;

    let len = fs::metadata(path)
        .map_err(|_| AudioError::FileCorrupted)?
        .len();

    let decoder = DecoderBuilder::new()
        .with_data(BufReader::new(file))
        .with_byte_len(len)
        .with_seekable(true)
        .with_coarse_seek(true)
        // .with_hint("mp3")
        .build()
        .map_err(|_| AudioError::FileCorrupted)?;

    // let decoder = decoder.repeat_infinite();
    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let duration = decoder.total_duration().unwrap_or(Duration::from_secs(0));

    let decoder = decoder.amplify_decibel(3.5);

    let filtered = LiveFilteredSource::new(decoder, lp_cutoff, hp_cutoff);
    let spatial = LiveSpatialSource::new(filtered, ms_mode, width, pan);
    player.append(spatial);

    Ok((duration, channels, sample_rate))
}

// V2
pub(crate) fn open_file_with_decoder(path: &Path) -> Result<Decoder<BufReader<File>>, AudioError> {
    let file = File::open(path)
        .map_err(|_| AudioError::FileNotFound(path.to_string_lossy().into_owned()))?;

    let len = fs::metadata(path)
        .map_err(|_| AudioError::FileCorrupted)?
        .len();

    let decoder = DecoderBuilder::new()
        .with_data(BufReader::new(file))
        .with_byte_len(len)
        .with_seekable(true)
        .with_coarse_seek(true)
        // .with_hint("mp3")
        .build()
        .map_err(|_| AudioError::FileCorrupted)?;

    Ok(decoder)
}

// V2
fn append_source<S>(state: &AudioState, source: S)
where
    S: Source + Send + 'static,
{
    let filtered = LiveFilteredSource::new(
        source,
        state.effects.low_pass.clone(),
        state.effects.high_pass.clone(),
    );

    let spatial = LiveSpatialSource::new(
        filtered,
        state.effects.mid_side.clone(),
        state.effects.stereo_width.clone(),
        state.effects.pan.clone(),
    );

    state.player.append(spatial);
    // state.player.set_speed(state.playback.speed);
}

// V2
pub(crate) fn build_audio_pipeline(
    state: &AudioState,
    mut decoder: Decoder<BufReader<File>>,
    position: Duration,
) -> Result<(), AudioError> {
    // seek before wsola
    decoder
        .try_seek(position)
        .map_err(|_| AudioError::SeekFailed)?;

    let decoder = decoder.amplify_decibel(3.5);

    if state.playback.preserve_pitch {
        // Decoder → WSOLA → Filters → Spatial
        // let mut wsola_source = rodio_wsola::Wsola::new(decoder, 1.0);

        let source = TimeStretchSource::new(decoder, state.playback.speed_shared.clone())
            .map_err(|_| AudioError::RenderAudioFailed)?;

        append_source(state, source);

        // TimestretchSource controls the speed.
        // state.player.set_speed(1.0);

        /* let wsola_source = decoder.wsola(state.playback.speed);
        // wsola_source.set_speed(state.playback.speed);
        append_source(state, wsola_source);

        // WSOLA controls the playback rate.
        state.player.set_speed(1.0); */
    } else {
        // Decoder → Filters → Spatial
        let source = decoder.speed(state.playback.speed);
        append_source(state, source);
    }

    Ok(())
}

/// Открывает файл, декодирует все сэмплы в память.
/// Возвращает (моно-сэмплы, sample_rate, duration).
pub fn load_samples_mono(
    path: &Path,
    limit_secs: u32,
) -> Result<(Vec<f32>, u32, Duration), AudioError> {
    let file = File::open(path)
        .map_err(|_| AudioError::FileNotFound(path.to_string_lossy().into_owned()))?;

    let len = fs::metadata(path)
        .map_err(|_| AudioError::FileCorrupted)?
        .len();

    let decoder = DecoderBuilder::new()
        .with_data(BufReader::new(file))
        .with_byte_len(len)
        // .with_hint("mp3")
        .build()
        .map_err(|_| AudioError::FileCorrupted)?;

    let sample_rate = decoder.sample_rate().get();
    let channels = decoder.channels().get() as usize;
    let duration = decoder.total_duration().unwrap_or(Duration::from_secs(0));

    let max_samples = sample_rate as usize * channels * limit_secs as usize;

    // normalize
    let decoder = decoder.amplify_decibel(3.5);

    // Collect all samples and mix to mono
    let mut mono_samples: Vec<f32> = Vec::new();
    let mut frame_buf = vec![0.0f32; channels];

    for (i, sample) in decoder.enumerate() {
        if i >= max_samples {
            break; // <-- лимит
        }

        let ch = i % channels;
        frame_buf[ch] = sample as f32;

        if ch == channels - 1 {
            let mono: f32 = frame_buf.iter().sum::<f32>() / channels as f32;
            mono_samples.push(mono);
        }
    }

    if mono_samples.is_empty() {
        return Err(AudioError::FileCorrupted);
    }

    Ok((mono_samples, sample_rate, duration))
}
