use super::filters::LiveFilteredSource;
use super::spatial::LiveSpatialSource;
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

use rodio::{Player, decoder::DecoderBuilder};
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

    let channels = decoder.channels();
    let sample_rate = decoder.sample_rate();
    let duration = decoder.total_duration().unwrap_or(Duration::from_secs(0));

    let filtered = LiveFilteredSource::new(decoder, lp_cutoff, hp_cutoff);
    let spatial = LiveSpatialSource::new(filtered, ms_mode, width, pan);
    player.append(spatial);

    Ok((duration, channels, sample_rate))
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
