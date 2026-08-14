pub mod bpm_key;
pub mod waveform;

pub(crate) struct AnalysisResult {
    pub bpm: f32,
    pub key_name: String,
    pub key_numerical: String,
    pub beat_grid: BeatGrid,
}

pub(crate) struct BeatGrid {
    pub offset: f32,
    pub beats: Vec<f32>,
    pub bars: Vec<f32>,
    pub downbeats: Vec<f32>,
}
