use rodio::Source;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Smoothing coefficient for gain changes (~10 ms at common sample rates).
const GAIN_SMOOTHING: f32 = 0.002;

/// Applies a live linear gain to a source.
///
/// The gain factor is read from shared state on every sample, so preamp and
/// loudness normalization changes are audible without rebuilding the pipeline.
pub(crate) struct LiveGainSource<S> {
    inner: S,
    gain: Arc<AtomicU32>,
    current_gain: f32,
}

impl<S: Source> LiveGainSource<S> {
    pub(crate) fn new(inner: S, gain: Arc<AtomicU32>) -> Self {
        let current_gain = f32::from_bits(gain.load(Ordering::Relaxed));

        Self {
            inner,
            gain,
            current_gain,
        }
    }
}

impl<S: Source> Iterator for LiveGainSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        let target = f32::from_bits(self.gain.load(Ordering::Relaxed));

        self.current_gain += (target - self.current_gain) * GAIN_SMOOTHING;

        Some(sample * self.current_gain)
    }
}

impl<S: Source> Source for LiveGainSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> NonZeroU16 {
        self.inner.channels()
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(position)
    }
}

#[cfg(test)]
mod tests {
    use super::{GAIN_SMOOTHING, LiveGainSource};
    use rodio::buffer::SamplesBuffer;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn live_gain_should_scale_samples() {
        let gain = Arc::new(AtomicU32::new(2.0_f32.to_bits()));
        let source = SamplesBuffer::new(
            NonZeroU16::new(1).unwrap(),
            NonZeroU32::new(48_000).unwrap(),
            vec![0.25_f32; 64],
        );
        let mut source = LiveGainSource::new(source, gain);

        let samples: Vec<f32> = source.by_ref().take(64).collect();

        assert!(samples.iter().all(|sample| (sample - 0.5).abs() < 1e-3));
    }

    #[test]
    fn live_gain_should_smooth_changes() {
        let gain = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let source = SamplesBuffer::new(
            NonZeroU16::new(1).unwrap(),
            NonZeroU32::new(48_000).unwrap(),
            vec![0.25_f32; 2_000],
        );
        let mut source = LiveGainSource::new(source, Arc::clone(&gain));

        gain.store(2.0_f32.to_bits(), Ordering::Relaxed);

        let samples: Vec<f32> = source.by_ref().take(2_000).collect();

        assert!(samples[0] > 0.25 && samples[0] < 0.251);
        assert!(samples[1_999] > samples[0]);
        assert!(samples[1_999] < 0.5);
    }

    #[test]
    fn gain_smoothing_should_be_small_per_sample() {
        assert!(GAIN_SMOOTHING > 0.0 && GAIN_SMOOTHING < 0.01);
    }
}
