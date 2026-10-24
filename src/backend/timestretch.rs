use std::collections::VecDeque;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use rodio::Source;
use timestretch::engine::{
    Engine, EngineConfig, EngineController, EngineProcessor, EngineProfile, SourceProducer,
};

const INPUT_FRAMES_PER_BLOCK: usize = 1024;
const OUTPUT_FRAMES_PER_BLOCK: usize = 1024;
const MAX_DRAIN_BLOCKS: usize = 32;
const MIN_SPEED: f32 = 0.25;
const MAX_SPEED: f32 = 2.0;

pub(crate) struct TimeStretchSource<S> {
    inner: S,

    controller: EngineController,
    processor: EngineProcessor,
    source: SourceProducer,

    output: VecDeque<f32>,
    input_block: Vec<f32>,
    processed_block: Vec<f32>,

    speed: Arc<AtomicU32>,

    channels: NonZeroU16,
    sample_rate: NonZeroU32,

    input_finished: bool,
    finish_requested: bool,
    engine_drained: bool,
    drain_underruns: u64,
    drain_blocks: usize,

    preserve_pitch: Arc<AtomicBool>,
    current_profile: EngineProfile,
}

impl<S: Source> TimeStretchSource<S> {
    pub(crate) fn new(
        inner: S,
        speed: Arc<AtomicU32>,
        preserve_pitch: Arc<AtomicBool>,
    ) -> Result<Self, timestretch::StretchError> {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate();

        let current_profile = if preserve_pitch.load(Ordering::Relaxed) {
            EngineProfile::WideKeylock
        } else {
            EngineProfile::Tape
        };

        let initial_speed =
            f32::from_bits(speed.load(Ordering::Acquire)).clamp(MIN_SPEED, MAX_SPEED);

        let (controller, processor, source) = Self::build_engine(
            sample_rate.get(),
            channels.get() as usize,
            current_profile,
            initial_speed as f64,
        )?;

        let channels_count = channels.get() as usize;

        Ok(Self {
            inner,
            controller,
            processor,
            source,

            output: VecDeque::with_capacity(OUTPUT_FRAMES_PER_BLOCK * channels_count * 4),
            input_block: Vec::with_capacity(INPUT_FRAMES_PER_BLOCK * channels_count),
            processed_block: vec![0.0; OUTPUT_FRAMES_PER_BLOCK * channels_count],

            speed,
            preserve_pitch,
            current_profile,

            channels,
            sample_rate,

            input_finished: false,
            finish_requested: false,
            engine_drained: false,
            drain_underruns: 0,
            drain_blocks: 0,
        })
    }

    fn expected_profile(&self) -> EngineProfile {
        if self.preserve_pitch.load(Ordering::Relaxed) {
            EngineProfile::WideKeylock
        } else {
            EngineProfile::Tape
        }
    }

    fn build_engine(
        sample_rate: u32,
        channels: usize,
        profile: EngineProfile,
        initial_tempo_rate: f64,
    ) -> Result<(EngineController, EngineProcessor, SourceProducer), timestretch::StretchError>
    {
        let handles = Engine::build(EngineConfig {
            sample_rate,
            channels,
            profile,
            initial_tempo_rate,
            ..EngineConfig::default()
        })?;

        Ok((handles.controller, handles.processor, handles.source))
    }

    fn update_engine(&mut self) {
        let expected_profile = self.expected_profile();

        if self.current_profile == expected_profile {
            return;
        }

        let Ok((controller, processor, source)) = Self::build_engine(
            self.sample_rate.get(),
            self.channels.get() as usize,
            expected_profile,
            self.current_speed() as f64,
        ) else {
            return;
        };

        self.controller = controller;
        self.processor = processor;
        self.source = source;
        self.current_profile = expected_profile;

        self.output.clear();
        self.input_block.clear();
        self.processed_block.fill(0.0);

        self.input_finished = false;
        self.finish_requested = false;
        self.engine_drained = false;
        self.drain_underruns = 0;
        self.drain_blocks = 0;
    }

    fn current_speed(&self) -> f32 {
        f32::from_bits(self.speed.load(Ordering::Acquire)).clamp(MIN_SPEED, MAX_SPEED)
    }

    fn update_speed(&self) {
        self.controller.set_tempo_rate(self.current_speed() as f64);
    }

    fn process_block(&mut self) {
        self.processed_block.fill(0.0);
        self.processor.process(&mut self.processed_block);

        self.output.extend(self.processed_block.iter().copied());
    }

    fn fill_input_buffer(&mut self) {
        let channels = self.channels.get() as usize;

        let required_frames = self
            .source
            .demand_hint(OUTPUT_FRAMES_PER_BLOCK, self.current_speed() as f64);

        while !self.input_finished && self.source.occupied_frames() < required_frames {
            self.input_block.clear();

            let missing_frames = required_frames.saturating_sub(self.source.occupied_frames());

            let target_samples = (missing_frames * channels).min(INPUT_FRAMES_PER_BLOCK * channels);

            while self.input_block.len() < target_samples {
                match self.inner.next() {
                    Some(sample) => self.input_block.push(sample),
                    None => {
                        self.input_finished = true;
                        break;
                    }
                }
            }

            if self.input_block.is_empty() {
                break;
            }

            let mut offset = 0;
            let mut stalls = 0;

            while offset < self.input_block.len() {
                let accepted_frames = self.source.push(&self.input_block[offset..]);

                let accepted_samples = accepted_frames * channels;

                if accepted_samples == 0 {
                    // Ring-buffer заполнен. Освобождаем его, обработав один
                    // выходной блок, и повторяем попытку.
                    self.process_block();
                    stalls += 1;

                    // A stuck engine must not spin the audio thread forever.
                    if stalls >= MAX_DRAIN_BLOCKS {
                        self.engine_drained = true;
                        return;
                    }

                    continue;
                }

                stalls = 0;
                offset += accepted_samples;
            }
        }
    }

    fn fill_output(&mut self) {
        if self.engine_drained {
            return;
        }

        self.update_engine();
        self.update_speed();

        self.fill_input_buffer();

        if self.input_finished && !self.finish_requested {
            self.finish_requested = self.source.finish();

            if self.finish_requested {
                self.drain_underruns = self.controller.underrun_frames();
            }
        }

        self.process_block();

        if self.input_finished && self.finish_requested {
            self.drain_blocks += 1;

            // A new underrun after the final padding means the engine has
            // consumed everything real. The ring can stay non-empty forever
            // while the pipeline holds the tail, so use a bounded fallback.
            let terminal_underrun = self.controller.underrun_frames() > self.drain_underruns;

            if terminal_underrun || self.drain_blocks >= MAX_DRAIN_BLOCKS {
                self.engine_drained = true;
            }
        }
    }
}

impl<S: Source> Iterator for TimeStretchSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(sample) = self.output.pop_front() {
                return Some(sample);
            }

            if self.engine_drained {
                return None;
            }

            self.fill_output();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TimeStretchSource;
    use rodio::buffer::SamplesBuffer;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32};

    fn sine(sample_rate: u32, frames: usize) -> SamplesBuffer {
        let mut samples = Vec::with_capacity(frames * 2);

        for frame in 0..frames {
            let value =
                (frame as f32 / sample_rate as f32 * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            samples.extend_from_slice(&[value, value]);
        }

        SamplesBuffer::new(
            NonZeroU16::new(2).unwrap(),
            NonZeroU32::new(sample_rate).unwrap(),
            samples,
        )
    }

    fn source_should_finish(
        sample_rate: u32,
        frames: usize,
        speed_value: f32,
        preserve_pitch: bool,
    ) {
        let speed = Arc::new(AtomicU32::new(speed_value.to_bits()));
        let pitch = Arc::new(AtomicBool::new(preserve_pitch));
        let mut source = TimeStretchSource::new(sine(sample_rate, frames), speed, pitch).unwrap();

        let limit = frames * 16 + sample_rate as usize * 4;
        let mut finished = false;

        for _ in 0..limit {
            if source.next().is_none() {
                finished = true;
                break;
            }
        }

        assert!(
            finished,
            "source never finished at {sample_rate} Hz, {frames} frames, speed={speed_value}, preserve_pitch={preserve_pitch}"
        );
    }

    #[test]
    fn time_stretch_should_finish_at_common_rates() {
        for sample_rate in [44_100, 48_000] {
            for preserve_pitch in [false, true] {
                source_should_finish(sample_rate, sample_rate as usize + 1, 1.0, preserve_pitch);
            }
        }

        source_should_finish(48_000, 31_337, 1.25, false);
        source_should_finish(48_000, 31_337, 0.75, true);
    }
}

impl<S: Source> Source for TimeStretchSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        // Ограниченный span нужен rodio, чтобы периодически заново
        // проверять sample rate источника при переходе между треками.
        Some(OUTPUT_FRAMES_PER_BLOCK * self.channels.get() as usize)
    }

    fn channels(&self) -> NonZeroU16 {
        self.channels
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let speed = self.current_speed();

        self.inner
            .total_duration()
            .map(|duration| duration.mul_f32(1.0 / speed))
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(position)?;

        let (controller, processor, source) = Self::build_engine(
            self.sample_rate.get(),
            self.channels.get() as usize,
            self.current_profile,
            self.current_speed() as f64,
        )
        .map_err(|_| rodio::source::SeekError::NotSupported {
            underlying_source: "failed to rebuild timestretch engine",
        })?;

        self.controller = controller;
        self.processor = processor;
        self.source = source;

        self.output.clear();
        self.input_block.clear();
        self.processed_block.fill(0.0);

        self.input_finished = false;
        self.finish_requested = false;
        self.engine_drained = false;
        self.drain_underruns = 0;
        self.drain_blocks = 0;

        Ok(())
    }
}
