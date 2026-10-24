use rodio::Source;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const WINDOW_MILLIS: u64 = 30;
const MIN_LEVEL: f32 = 1.0e-6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MeterReading {
    pub(crate) peak: f32,
    pub(crate) rms: f32,
}

#[derive(Clone)]
pub(crate) struct MeterState {
    peak: Arc<AtomicU32>,
    rms: Arc<AtomicU32>,
    generation: Arc<AtomicU32>,
}

impl MeterState {
    pub(crate) fn new() -> Self {
        Self {
            peak: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            rms: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            generation: Arc::new(AtomicU32::new(0)),
        }
    }

    pub(crate) fn reset(&self) {
        self.peak.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.rms.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn read(&self) -> MeterReading {
        MeterReading {
            peak: f32::from_bits(self.peak.swap(0.0f32.to_bits(), Ordering::Relaxed)),
            rms: f32::from_bits(self.rms.load(Ordering::Relaxed)),
        }
    }

    fn generation(&self) -> u32 {
        self.generation.load(Ordering::Relaxed)
    }

    fn publish(&self, peak: f32, rms: f32) {
        self.peak
            .fetch_max(sanitize_level(peak).to_bits(), Ordering::Relaxed);

        self.rms
            .store(sanitize_level(rms).to_bits(), Ordering::Relaxed);
    }
}

pub(crate) struct MeteredSource<S> {
    inner: S,
    meter: MeterState,

    channels: NonZeroU16,
    sample_rate: NonZeroU32,
    window_samples: usize,

    generation: u32,
    sample_count: usize,
    peak: f32,
    sum_squares: f64,
}

impl<S: Source> MeteredSource<S> {
    pub(crate) fn new(inner: S, meter: MeterState) -> Self {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate();

        let window_samples = u64::from(sample_rate.get())
            .saturating_mul(u64::from(channels.get()))
            .saturating_mul(WINDOW_MILLIS)
            .checked_div(1_000)
            .unwrap_or(1)
            .max(1) as usize;

        Self {
            generation: meter.generation(),
            inner,
            meter,
            channels,
            sample_rate,
            window_samples,
            sample_count: 0,
            peak: 0.0,
            sum_squares: 0.0,
        }
    }

    fn reset_window(&mut self) {
        self.sample_count = 0;
        self.peak = 0.0;
        self.sum_squares = 0.0;
    }

    fn sync_generation(&mut self) {
        let generation = self.meter.generation();

        if generation != self.generation {
            self.reset_window();
            self.generation = generation;
        }
    }

    fn observe(&mut self, sample: f32) {
        self.sync_generation();

        let sample = if sample.is_finite() { sample } else { 0.0 };
        let absolute = sample.abs();

        self.peak = self.peak.max(absolute);
        self.sum_squares += f64::from(sample) * f64::from(sample);
        self.sample_count += 1;

        if self.sample_count >= self.window_samples {
            self.publish_window();
        }
    }

    fn publish_window(&mut self) {
        if self.sample_count == 0 {
            return;
        }

        let rms = (self.sum_squares / self.sample_count as f64).sqrt() as f32;

        self.meter.publish(self.peak, rms);
        self.reset_window();
    }
}

impl<S: Source> Iterator for MeteredSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(sample) => {
                self.observe(sample);
                Some(sample)
            }
            None => {
                self.publish_window();
                None
            }
        }
    }
}

impl<S: Source> Source for MeteredSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> NonZeroU16 {
        self.channels
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(position)?;

        self.meter.reset();
        self.reset_window();
        self.generation = self.meter.generation();

        Ok(())
    }
}

pub(crate) fn linear_to_dbfs(value: f32) -> f32 {
    let value = if value.is_finite() {
        value.max(MIN_LEVEL)
    } else {
        MIN_LEVEL
    };

    let db = 20.0 * value.log10();

    if db.is_finite() {
        db.clamp(-60.0, 0.0)
    } else {
        -60.0
    }
}

fn sanitize_level(value: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}
