use std::collections::VecDeque;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use rodio::Source;
use timestretch::engine::{
    Engine, EngineConfig, EngineController, EngineProcessor, EngineProfile, SourceProducer,
};

const INPUT_FRAMES_PER_BLOCK: usize = 1024;
const OUTPUT_FRAMES_PER_BLOCK: usize = 1024;
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
}

impl<S: Source> TimeStretchSource<S> {
    pub(crate) fn new(inner: S, speed: Arc<AtomicU32>) -> Result<Self, timestretch::StretchError> {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate();

        let (controller, processor, source) =
            Self::build_engine(sample_rate.get(), channels.get() as usize)?;

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

            channels,
            sample_rate,

            input_finished: false,
            finish_requested: false,
            engine_drained: false,
        })
    }

    fn build_engine(
        sample_rate: u32,
        channels: usize,
    ) -> Result<(EngineController, EngineProcessor, SourceProducer), timestretch::StretchError>
    {
        let handles = Engine::build(EngineConfig {
            sample_rate,
            channels,
            profile: EngineProfile::WideKeylock,
            initial_tempo_rate: 1.0,
            ..EngineConfig::default()
        })?;

        Ok((handles.controller, handles.processor, handles.source))
    }

    fn current_speed(&self) -> f32 {
        f32::from_bits(self.speed.load(Ordering::Relaxed)).clamp(MIN_SPEED, MAX_SPEED)
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

            while offset < self.input_block.len() {
                let accepted_frames = self.source.push(&self.input_block[offset..]);

                let accepted_samples = accepted_frames * channels;

                if accepted_samples == 0 {
                    // Ring-buffer заполнен. Освобождаем его, обработав один
                    // выходной блок, и повторяем попытку.
                    self.process_block();
                    continue;
                }

                offset += accepted_samples;
            }
        }
    }

    fn fill_output(&mut self) {
        if self.engine_drained {
            return;
        }

        self.update_speed();

        self.fill_input_buffer();

        if self.input_finished && !self.finish_requested {
            self.finish_requested = self.source.finish();
        }

        self.process_block();

        if self.input_finished && self.finish_requested && self.source.occupied_frames() == 0 {
            self.engine_drained = true;
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

impl<S: Source> Source for TimeStretchSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
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

        let (controller, processor, source) =
            Self::build_engine(self.sample_rate.get(), self.channels.get() as usize).map_err(
                |_| rodio::source::SeekError::NotSupported {
                    underlying_source: "failed to rebuild timestretch engine",
                },
            )?;

        self.controller = controller;
        self.processor = processor;
        self.source = source;

        self.output.clear();
        self.input_block.clear();
        self.processed_block.fill(0.0);

        self.input_finished = false;
        self.finish_requested = false;
        self.engine_drained = false;

        Ok(())
    }
}

/* use std::collections::VecDeque;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use rodio::Source;
use timestretch::engine::{
    Engine, EngineConfig, EngineController, EngineProcessor, EngineProfile, SourceProducer,
};

// Блоки по 2048 фреймов гарантируют мягкий звук в Keylock режиме
const INPUT_FRAMES_PER_BLOCK: usize = 2048;
const OUTPUT_FRAMES_PER_BLOCK: usize = 2048;
const MIN_SPEED: f32 = 0.25;
const MAX_SPEED: f32 = 2.0;

pub(crate) struct TimeStretchSource<S> {
    inner: S,

    controller: EngineController,
    processor: EngineProcessor,
    source: SourceProducer,
    current_profile: EngineProfile, // Запоминаем текущий профиль

    output: VecDeque<f32>,
    input_block: Vec<f32>,
    processed_block: Vec<f32>,

    speed: Arc<AtomicU32>,
    preserve_pitch: Arc<AtomicBool>, // Атомик для переключения режима на лету

    channels: NonZeroU16,
    sample_rate: NonZeroU32,

    input_finished: bool,
    finish_requested: bool,
    engine_drained: bool,
}

impl<S: Source> TimeStretchSource<S> {
    pub(crate) fn new(
        inner: S,
        speed: Arc<AtomicU32>,
        preserve_pitch: Arc<AtomicBool>,
    ) -> Result<Self, timestretch::StretchError> {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate();

        // Считываем начальное состояние режима
        let initial_preserve = preserve_pitch.load(Ordering::Relaxed);
        let initial_profile = if initial_preserve {
            EngineProfile::WideKeylock
        } else {
            EngineProfile::Tape
        };

        let (controller, processor, source) =
            Self::build_engine(sample_rate.get(), channels.get() as usize, initial_profile)?;

        let channels_count = channels.get() as usize;

        Ok(Self {
            inner,
            controller,
            processor,
            source,
            current_profile: initial_profile,

            output: VecDeque::with_capacity(OUTPUT_FRAMES_PER_BLOCK * channels_count * 4),
            input_block: Vec::with_capacity(INPUT_FRAMES_PER_BLOCK * channels_count),
            processed_block: vec![0.0; OUTPUT_FRAMES_PER_BLOCK * channels_count],

            speed,
            preserve_pitch,

            channels,
            sample_rate,

            input_finished: false,
            finish_requested: false,
            engine_drained: false,
        })
    }

    fn build_engine(
        sample_rate: u32,
        channels: usize,
        profile: EngineProfile,
    ) -> Result<(EngineController, EngineProcessor, SourceProducer), timestretch::StretchError>
    {
        let handles = Engine::build(EngineConfig {
            sample_rate,
            channels,
            profile,
            initial_tempo_rate: 1.0,
            ..EngineConfig::default()
        })?;

        Ok((handles.controller, handles.processor, handles.source))
    }

    fn current_speed(&self) -> f32 {
        f32::from_bits(self.speed.load(Ordering::Relaxed)).clamp(MIN_SPEED, MAX_SPEED)
    }

    fn update_speed(&mut self) {
        let target_preserve = self.preserve_pitch.load(Ordering::Relaxed);
        let expected_profile = if target_preserve {
            EngineProfile::WideKeylock
        } else {
            EngineProfile::Tape
        };

        // --- ГОРЯЧЕЕ ПЕРЕКЛЮЧЕНИЕ РЕЖИМА ПИТЧА ---
        // Если пользователь переключил тумблер в GUI, мы пересобираем только движок timestretch.
        // Фильтры и Rodio снаружи ничего не замечают, воспроизведение не прерывается!
        if self.current_profile != expected_profile {
            if let Ok((c, p, s)) = Self::build_engine(
                self.sample_rate.get(),
                self.channels.get() as usize,
                expected_profile,
            ) {
                self.controller = c;
                self.processor = p;
                self.source = s;
                self.current_profile = expected_profile;

                // Мягко очищаем внутренние буферы, чтобы звук мгновенно переключился без щелчка
                self.output.clear();
                self.input_block.clear();
            }
        }

        // В обоих профилях скорость регулируется через set_tempo_rate
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
            while offset < self.input_block.len() {
                let accepted_frames = self.source.push(&self.input_block[offset..]);
                let accepted_samples = accepted_frames * channels;

                if accepted_samples == 0 {
                    self.process_block();
                    continue;
                }

                offset += accepted_samples;
            }
        }
    }

    fn fill_output(&mut self) {
        if self.engine_drained {
            return;
        }

        self.update_speed();
        self.fill_input_buffer();

        if self.input_finished && !self.finish_requested {
            self.finish_requested = self.source.finish();
        }

        // Фикс робо-звука: рендерим блок только если в выходной очереди пусто
        if self.output.is_empty() && (self.source.occupied_frames() > 0 || self.finish_requested) {
            self.process_block();
        }

        if self.input_finished
            && self.finish_requested
            && self.source.occupied_frames() == 0
            && self.output.is_empty()
        {
            self.engine_drained = true;
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

            if self.output.is_empty() && self.engine_drained {
                return None;
            }
        }
    }
}

impl<S: Source> Source for TimeStretchSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> NonZeroU16 {
        self.channels
    }
    fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        let speed = self.current_speed();
        if speed <= 0.01 {
            return None;
        }

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

        Ok(())
    }
} */
