use crate::shared::event_types::{AudioError, BeatGridData};

use stratum_dsp::{AnalysisConfig, analyze_audio};

/// Result of BPM and key analysis
#[derive(Clone, Debug)]
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
    if samples.is_empty() {
        return Err(AudioError::FileCorrupted);
    }

    let ch = channels as usize;

    // Mix to mono on the fly
    let mut mono_samples: Vec<f32> = Vec::with_capacity(samples.len() / ch);
    for frame in samples.chunks_exact(ch) {
        let mono: f32 = frame.iter().sum::<f32>() / ch as f32;
        mono_samples.push(mono);
    }

    // Run the analysis
    let result = analyze_audio(&mono_samples, sample_rate, AnalysisConfig::default())
        .map_err(|_| AudioError::FileCorrupted)?;

    let beat_offset = result.beat_grid.downbeats.first().copied().unwrap_or(0.0);

    /* let conf = compute_confidence(&result);

    println!(
        "BPM: {:.2} (confidence: {:.2})",
        result.bpm, conf.bpm_confidence
    );
    println!("Key: {} ({})", result.key.name(), result.key.numerical()); // e.g., "C major (1A)", "Am (1B)"
    println!("Beat grid stability: {:.2}", result.grid_stability); */

    // println!("Beat Grid Bars: {:#?}", result.beat_grid.bars);
    // println!("Beat Grid Beats: {:#?}", result.beat_grid.beats);
    // println!("Beat Grid Downbeats: {:#?}", result.beat_grid.downbeats);

    // println!("BPM: {:#?}", result.bpm);
    // println!("BPM Confidence: {:#?}", result.bpm_confidence);

    let beat_grid = BeatGridData {
        offset: beat_offset,
        beats: result.beat_grid.beats.clone(),
        bars: result.beat_grid.bars.clone(),
        downbeats: result.beat_grid.downbeats.clone(),
    };

    Ok(BpmKeyResult {
        bpm: result.bpm.round(), // Round to 1 decimal
        beat_grid,
        key_name: result.key.name(),
        key_numerical: result.key.numerical(),
    })
}
