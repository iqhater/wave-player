use crate::shared::event_types::{AudioError, BeatGridData};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use stratum_dsp::{AnalysisConfig, analyze_audio};

/// Result of BPM and key analysis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BpmKeyResult {
    pub bpm: f32,
    pub beat_grid: BeatGridData,
    pub key_name: String,
    pub key_numerical: String,
}

/// Analyze raw audio samples to detect BPM and musical key.
/// `samples` — interleaved f32 audio data.
pub fn analyze_bpm_key(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<BpmKeyResult, AudioError> {
    if samples.is_empty() || sample_rate == 0 || channels == 0 {
        return Err(AudioError::FileCorrupted);
    }

    // The analysis worker already decodes mono PCM.
    let mono: Cow<'_, [f32]> = if channels == 1 {
        Cow::Borrowed(samples)
    } else {
        Cow::Owned(
            samples
                .chunks_exact(channels as usize)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect(),
        )
    };

    let result = analyze_audio(&mono, sample_rate, AnalysisConfig::default())
        .map_err(|_| AudioError::FileCorrupted)?;

    if !result.bpm.is_finite() {
        return Err(AudioError::FileCorrupted);
    }

    let offset = result.beat_grid.downbeats.first().copied().unwrap_or(0.0);

    Ok(BpmKeyResult {
        bpm: result.bpm.round(),
        key_name: result.key.name(),
        key_numerical: result.key.numerical(),
        beat_grid: BeatGridData {
            offset,
            beats: result.beat_grid.beats,
            bars: result.beat_grid.bars,
            downbeats: result.beat_grid.downbeats,
        },
    })
}
