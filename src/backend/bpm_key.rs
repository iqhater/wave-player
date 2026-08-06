use crate::shared::event_types::AudioError;
use stratum_dsp::{AnalysisConfig, analyze_audio};

/// Result of BPM and key analysis
#[derive(Clone, Debug, PartialEq)]
pub struct BpmKeyResult {
    pub bpm: f32,
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

    /* let conf = compute_confidence(&result);

    println!(
        "BPM: {:.2} (confidence: {:.2})",
        result.bpm, conf.bpm_confidence
    );
    println!("Key: {} ({})", result.key.name(), result.key.numerical()); // e.g., "C major (1A)", "Am (1B)"
    println!("Beat grid stability: {:.2}", result.grid_stability); */

    Ok(BpmKeyResult {
        // bpm: (result.bpm * 10.0).round() / 10.0, // Round to 1 decimal
        bpm: result.bpm.round(), // Round to 1 decimal
        key_name: result.key.name(),
        key_numerical: result.key.numerical(),
    })
}
