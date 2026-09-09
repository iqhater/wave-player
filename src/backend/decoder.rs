use super::filters::LiveFilteredSource;
use super::spatial::LiveSpatialSource;
use super::state::AudioState;
use crate::shared::event_types::AudioError;
use rodio::Source;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use crate::backend::timestretch::TimeStretchSource;
use rodio::{Decoder, decoder::DecoderBuilder};

use std::fs;

// ---------------------------------------------------------------------------
// Decoding helpers
// ---------------------------------------------------------------------------

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

    let source = TimeStretchSource::new(
        decoder,
        state.playback.speed_shared.clone(),
        state.playback.preserve_pitch_shared.clone(),
    )
    .map_err(|_| AudioError::RenderAudioFailed)?;

    append_source(state, source);

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
