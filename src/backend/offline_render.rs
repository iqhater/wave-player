use std::path::{Path, PathBuf};
use std::time::Duration;

use hound::{SampleFormat, WavSpec, WavWriter};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use rodio::Source;
use timestretch::engine::{Engine, EngineConfig, EngineProfile};

use crate::backend::decoder::open_file_with_decoder;
use crate::shared::event_types::AudioError;

const NORMALIZATION_DB: f32 = 3.5;
const FILTER_Q: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderMidSideMode {
    Stereo,
    Mid,
    Side,
}

#[derive(Clone, Debug)]
pub(crate) struct RenderSettings {
    pub(crate) input_path: PathBuf,
    pub(crate) output_path: PathBuf,

    pub(crate) loop_start: Duration,
    pub(crate) loop_end: Duration,

    pub(crate) speed: f32,
    pub(crate) preserve_pitch: bool,

    pub(crate) low_pass: Option<f32>,
    pub(crate) high_pass: Option<f32>,

    pub(crate) mid_side_mode: RenderMidSideMode,
    pub(crate) stereo_width: f32,
    pub(crate) pan: f32,
}

#[derive(Clone, Copy, Debug)]
struct StereoFrame {
    left: f32,
    right: f32,
}

pub(crate) fn render_loop(
    settings: &RenderSettings,
    mut report_progress: impl FnMut(f32),
) -> Result<(), AudioError> {
    validate_settings(settings)?;

    report_progress(0.10);

    let (input_frames, sample_rate) = decode_loop(settings)?;

    report_progress(0.30);

    let timed_frames = if settings.preserve_pitch {
        apply_wsola(&input_frames, sample_rate, settings.speed)?
    } else {
        apply_resampling(&input_frames, settings.speed)
    };

    report_progress(0.55);

    let filtered_frames = apply_filters(
        timed_frames,
        sample_rate,
        settings.low_pass,
        settings.high_pass,
    );

    report_progress(0.75);

    let spatial_frames = apply_spatial(
        filtered_frames,
        settings.mid_side_mode,
        settings.stereo_width,
        settings.pan,
    );

    report_progress(0.90);

    write_wav(&settings.output_path, sample_rate, &spatial_frames)?;

    report_progress(1.0);

    Ok(())
}

fn validate_settings(settings: &RenderSettings) -> Result<(), AudioError> {
    if settings.loop_end <= settings.loop_start {
        return Err(AudioError::RenderAudioFailed);
    }

    if !settings.speed.is_finite() || settings.speed <= 0.0 {
        return Err(AudioError::RenderAudioFailed);
    }

    if !settings.stereo_width.is_finite() || !settings.pan.is_finite() {
        return Err(AudioError::RenderAudioFailed);
    }

    if !settings.input_path.is_file() {
        return Err(AudioError::FileNotFound(
            settings.input_path.to_string_lossy().into_owned(),
        ));
    }

    Ok(())
}

fn decode_loop(settings: &RenderSettings) -> Result<(Vec<StereoFrame>, u32), AudioError> {
    let mut decoder = open_file_with_decoder(&settings.input_path)?;

    let source_channels = decoder.channels().get() as usize;
    let sample_rate = decoder.sample_rate().get();

    if source_channels == 0 {
        return Err(AudioError::FileCorrupted);
    }

    // Some decoders do not report a total duration; fall back to the loop end so
    // the requested region is still honoured instead of collapsing to zero.
    let track_duration = decoder.total_duration().unwrap_or(settings.loop_end);

    let start = settings.loop_start.min(track_duration);
    let end = settings.loop_end.min(track_duration);

    if end <= start {
        return Err(AudioError::RenderAudioFailed);
    }

    // Seeking to the very beginning is a no-op for a freshly opened decoder and
    // some backends reject it with `SeekError::NotSupported`. Only seek when the
    // loop actually starts past the beginning.
    if start > Duration::ZERO {
        decoder
            .try_seek(start)
            .map_err(|_| AudioError::SeekFailed)?;
    }

    let render_duration = end - start;

    let samples: Vec<f32> = decoder
        .amplify_decibel(NORMALIZATION_DB)
        .take_duration(render_duration)
        .collect();

    if samples.is_empty() {
        return Err(AudioError::FileCorrupted);
    }

    let frames = convert_to_stereo(&samples, source_channels);

    if frames.is_empty() {
        return Err(AudioError::FileCorrupted);
    }

    Ok((frames, sample_rate))
}

fn convert_to_stereo(samples: &[f32], channels: usize) -> Vec<StereoFrame> {
    if channels == 0 {
        return Vec::new();
    }

    samples
        .chunks(channels)
        .filter_map(|frame| {
            let first = *frame.first()?;

            let second = if channels >= 2 {
                frame.get(1).copied().unwrap_or(first)
            } else {
                first
            };

            Some(StereoFrame {
                left: first,
                right: second,
            })
        })
        .collect()
}

fn apply_resampling(input: &[StereoFrame], speed: f32) -> Vec<StereoFrame> {
    if input.is_empty() {
        return Vec::new();
    }

    if (speed - 1.0).abs() < f32::EPSILON {
        return input.to_vec();
    }

    let output_len = ((input.len() as f32) / speed).ceil().max(1.0) as usize;

    let mut output = Vec::with_capacity(output_len);

    for output_index in 0..output_len {
        let source_position = output_index as f32 * speed;
        let left = interpolate_channel(input, source_position, true);
        let right = interpolate_channel(input, source_position, false);

        output.push(StereoFrame { left, right });
    }

    output
}

fn interpolate_channel(frames: &[StereoFrame], position: f32, left_channel: bool) -> f32 {
    let last_index = frames.len().saturating_sub(1);

    if position >= last_index as f32 {
        let frame = frames[last_index];

        return if left_channel {
            frame.left
        } else {
            frame.right
        };
    }

    let index_a = position.floor() as usize;
    let index_b = (index_a + 1).min(last_index);
    let mix = position - index_a as f32;

    let value_a = if left_channel {
        frames[index_a].left
    } else {
        frames[index_a].right
    };

    let value_b = if left_channel {
        frames[index_b].left
    } else {
        frames[index_b].right
    };

    value_a + (value_b - value_a) * mix
}

fn apply_wsola(
    input: &[StereoFrame],
    sample_rate: u32,
    speed: f32,
) -> Result<Vec<StereoFrame>, AudioError> {
    if input.is_empty() {
        return Err(AudioError::FileCorrupted);
    }

    let speed = speed.clamp(0.25, 2.0);

    let interleaved: Vec<f32> = input
        .iter()
        .flat_map(|frame| [frame.left, frame.right])
        .collect();

    let handles = Engine::build(EngineConfig {
        sample_rate,
        channels: 2,
        profile: EngineProfile::WideKeylock,
        initial_tempo_rate: 1.0,
        ..EngineConfig::default()
    })
    .map_err(|_| AudioError::RenderAudioFailed)?;

    let controller = handles.controller;
    let mut processor = handles.processor;
    let mut source = handles.source;

    controller.set_tempo_rate(speed as f64);

    const BLOCK_FRAMES: usize = 1024;
    const CHANNELS: usize = 2;
    const BLOCK_SAMPLES: usize = BLOCK_FRAMES * CHANNELS;

    let expected_frames = ((input.len() as f64) / speed as f64).ceil() as usize;

    let mut processed = Vec::with_capacity(expected_frames * CHANNELS);
    let mut output_block = vec![0.0f32; BLOCK_SAMPLES];

    let mut offset = 0;

    while offset < interleaved.len() {
        let accepted_frames = source.push(&interleaved[offset..]);
        let accepted_samples = accepted_frames * CHANNELS;

        if accepted_samples == 0 {
            output_block.fill(0.0);
            processor.process(&mut output_block);
            processed.extend_from_slice(&output_block);
        } else {
            offset += accepted_samples;
        }
    }

    while !source.finish() {
        output_block.fill(0.0);
        processor.process(&mut output_block);
        processed.extend_from_slice(&output_block);
    }

    while processed.len() / CHANNELS < expected_frames {
        output_block.fill(0.0);
        processor.process(&mut output_block);
        processed.extend_from_slice(&output_block);
    }

    processed.truncate(expected_frames * CHANNELS);

    let frames = convert_to_stereo(&processed, CHANNELS);

    if frames.is_empty() {
        return Err(AudioError::RenderAudioFailed);
    }

    Ok(frames)
}

fn apply_filters(
    mut frames: Vec<StereoFrame>,
    sample_rate: u32,
    low_pass: Option<f32>,
    high_pass: Option<f32>,
) -> Vec<StereoFrame> {
    let sample_rate = sample_rate as f32;

    let mut low_pass_left = create_filter(BiquadFilterType::Lowpass, low_pass, sample_rate);

    let mut low_pass_right = create_filter(BiquadFilterType::Lowpass, low_pass, sample_rate);

    let mut high_pass_left = create_filter(BiquadFilterType::Highpass, high_pass, sample_rate);

    let mut high_pass_right = create_filter(BiquadFilterType::Highpass, high_pass, sample_rate);

    for frame in &mut frames {
        if let Some(filter) = low_pass_left.as_mut() {
            frame.left = filter.process(frame.left);
        }

        if let Some(filter) = low_pass_right.as_mut() {
            frame.right = filter.process(frame.right);
        }

        if let Some(filter) = high_pass_left.as_mut() {
            frame.left = filter.process(frame.left);
        }

        if let Some(filter) = high_pass_right.as_mut() {
            frame.right = filter.process(frame.right);
        }
    }

    frames
}

fn create_filter(
    filter_type: BiquadFilterType,
    cutoff: Option<f32>,
    sample_rate: f32,
) -> Option<Biquad<f32>> {
    let cutoff = cutoff?;

    if !cutoff.is_finite() || cutoff <= 0.0 {
        return None;
    }

    let max_cutoff = sample_rate * 0.45;
    let cutoff = cutoff.clamp(20.0, max_cutoff);

    Some(Biquad::new(filter_type, cutoff, sample_rate, FILTER_Q, 0.0))
}

fn apply_spatial(
    frames: Vec<StereoFrame>,
    mode: RenderMidSideMode,
    width: f32,
    pan: f32,
) -> Vec<StereoFrame> {
    let width = width.clamp(0.0, 2.0);
    let pan = pan.clamp(-1.0, 1.0);

    let pan_radian = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    let left_gain = pan_radian.cos();
    let right_gain = pan_radian.sin();

    frames
        .into_iter()
        .map(|frame| {
            let mid = (frame.left + frame.right) * 0.5;
            let side = (frame.left - frame.right) * 0.5;

            let adjusted_side = side * width;

            let stereo_left = mid + adjusted_side;
            let stereo_right = mid - adjusted_side;

            let (left, right) = match mode {
                RenderMidSideMode::Stereo => (stereo_left, stereo_right),
                RenderMidSideMode::Mid => (mid, mid),
                RenderMidSideMode::Side => (adjusted_side, -adjusted_side),
            };

            StereoFrame {
                left: left * left_gain,
                right: right * right_gain,
            }
        })
        .collect()
}

fn write_wav(path: &Path, sample_rate: u32, frames: &[StereoFrame]) -> Result<(), AudioError> {
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 24,              // 32
        sample_format: SampleFormat::Int, // Float
    };

    let mut writer = WavWriter::create(path, spec).map_err(|_| AudioError::RenderAudioFailed)?;

    const MAX_24_BIT: f32 = 8_388_607.0;

    for frame in frames {
        if !frame.left.is_finite() || !frame.right.is_finite() {
            return Err(AudioError::RenderAudioFailed);
        }

        let left_sample = (frame.left.clamp(-1.0, 1.0) * MAX_24_BIT).round() as i32;

        let right_sample = (frame.right.clamp(-1.0, 1.0) * MAX_24_BIT).round() as i32;

        writer
            .write_sample(left_sample)
            .map_err(|_| AudioError::RenderAudioFailed)?;

        writer
            .write_sample(right_sample)
            .map_err(|_| AudioError::RenderAudioFailed)?;
    }

    writer
        .finalize()
        .map_err(|_| AudioError::RenderAudioFailed)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

    /// Writes a short 24-bit stereo WAV that the decoder can read back.
    fn write_source_wav(directory: &Path, seconds: f32) -> PathBuf {
        const SAMPLE_RATE: u32 = 48_000;

        let path = directory.join("source.wav");

        let spec = WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 24,
            sample_format: SampleFormat::Int,
        };

        let mut writer = WavWriter::create(&path, spec).unwrap();

        let frames = (SAMPLE_RATE as f32 * seconds) as usize;

        for frame in 0..frames {
            let value =
                (frame as f32 / SAMPLE_RATE as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            let sample = (value * 8_388_607.0) as i32;

            writer.write_sample(sample).unwrap();
            writer.write_sample(sample).unwrap();
        }

        writer.finalize().unwrap();

        path
    }

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();

            let root = std::env::temp_dir().join(format!(
                "wave-player-render-{}-{unique}-{}",
                std::process::id(),
                NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
            ));

            std::fs::create_dir_all(&root).unwrap();

            Self { root }
        }

        fn settings(&self, loop_start: Duration, loop_end: Duration) -> RenderSettings {
            RenderSettings {
                input_path: write_source_wav(&self.root, 2.0),
                output_path: self.root.join("exported.wav"),

                loop_start,
                loop_end,

                speed: 1.0,
                preserve_pitch: false,

                low_pass: None,
                high_pass: None,

                mid_side_mode: RenderMidSideMode::Stereo,
                stereo_width: 1.0,
                pan: 0.0,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Regression: a loop starting at zero used to fail because the decoder was
    /// asked to seek to `Duration::ZERO`, which some backends reject.
    #[test]
    fn render_loop_should_export_loop_starting_at_zero() {
        let fixture = Fixture::new();
        let settings = fixture.settings(Duration::ZERO, Duration::from_millis(500));

        render_loop(&settings, |_| {}).unwrap();

        let metadata = std::fs::metadata(&settings.output_path).unwrap();
        assert!(
            metadata.len() > 44,
            "exported WAV should contain audio data"
        );
    }

    #[test]
    fn render_loop_should_export_loop_starting_past_zero() {
        let fixture = Fixture::new();
        let settings = fixture.settings(Duration::from_millis(500), Duration::from_millis(1_500));

        render_loop(&settings, |_| {}).unwrap();

        assert!(settings.output_path.is_file());
    }

    #[test]
    fn render_loop_should_reject_empty_loop() {
        let fixture = Fixture::new();
        let settings = fixture.settings(Duration::from_millis(500), Duration::from_millis(500));

        let result = render_loop(&settings, |_| {});

        assert!(matches!(result, Err(AudioError::RenderAudioFailed)));
    }
}
