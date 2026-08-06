// LP, HP, BP фильтры
//! Модуль фильтров аудиосигнала (Low-Pass / High-Pass) с live-управлением.
//!
//! Фильтры читают частоту среза из `Arc<AtomicU32>` на каждом сэмпле,
//! поэтому их можно менять «на лету» без перезапуска источника.
//! Значение `0` в атомике означает «фильтр выключен».

use rodio::Source;
use rodio::source::SeekError;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Тип биквадратного фильтра.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterType {
    LowPass,
    HighPass,
}

/// Биквадратный фильтр 2-го порядка (RBJ cookbook, Butterworth Q = 0.7071).
#[derive(Clone, Debug)]
pub struct BiquadFilter {
    filter_type: FilterType,
    sample_rate: f32,
    cutoff: f32,
    q: f32,

    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,

    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    pub fn new(filter_type: FilterType, sample_rate: f32, cutoff: f32) -> Self {
        let mut f = Self {
            filter_type,
            sample_rate,
            cutoff: cutoff.clamp(20.0, sample_rate * 0.45),
            q: 0.7071,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        };
        f.update_coefficients();
        f
    }

    pub fn set_cutoff(&mut self, cutoff: f32) {
        self.cutoff = cutoff.clamp(20.0, self.sample_rate * 0.45);
        self.update_coefficients();
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update_coefficients();
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    fn update_coefficients(&mut self) {
        let w0 = std::f32::consts::TAU * (self.cutoff / self.sample_rate);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * self.q);

        let a0;
        match self.filter_type {
            FilterType::LowPass => {
                a0 = 1.0 + alpha;
                self.b0 = (1.0 - cos_w0) * 0.5;
                self.b1 = 1.0 - cos_w0;
                self.b2 = (1.0 - cos_w0) * 0.5;
                self.a1 = -2.0 * cos_w0;
                self.a2 = 1.0 - alpha;
            }
            FilterType::HighPass => {
                a0 = 1.0 + alpha;
                self.b0 = (1.0 + cos_w0) * 0.5;
                self.b1 = -(1.0 + cos_w0);
                self.b2 = (1.0 + cos_w0) * 0.5;
                self.a1 = -2.0 * cos_w0;
                self.a2 = 1.0 - alpha;
            }
        }

        self.b0 /= a0;
        self.b1 /= a0;
        self.b2 /= a0;
        self.a1 /= a0;
        self.a2 /= a0;
    }

    #[inline(always)]
    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }
}

/// Шаг плавного включения/выключения фильтра (в долях за сэмпл).
///
/// Фильтр не переключается мгновенно между «выкл» и «вкл», а плавно
/// подмешивается/убирается за ~10 мс. Это убирает щелчки при переключении
/// и делает включение фильтра с высоким срезом (например, LP 20000 Гц)
/// практически незаметным на слух.
#[inline(always)]
fn ramp_toward(current: f32, target: f32, step: f32) -> f32 {
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

/// Обёртка над rodio-источником с live-управляемыми LP/HP фильтрами.
///
/// Частоты среза хранятся в `Arc<AtomicU32>` (в Hz, `0` = выключено).
/// Движок обновляет атомики по командам UI — фильтр подхватывает новое
/// значение на следующем сэмпле, поэтому эффект слышен сразу при вращении ручки.
#[derive(Clone)]
pub struct LiveFilteredSource<S> {
    inner: S,
    lp: BiquadFilter,
    hp: BiquadFilter,
    lp_cutoff: Arc<AtomicU32>,
    hp_cutoff: Arc<AtomicU32>,
    channels: NonZeroU16,
    /// Текущая доля подмешивания LP-фильтра (0.0..=1.0).
    lp_mix: f32,
    /// Текущая доля подмешивания HP-фильтра (0.0..=1.0).
    hp_mix: f32,
    /// Шаг изменения mix за один сэмпл (~10 мс плавного перехода).
    ramp_step: f32,
    /// Последняя применённая частота среза LP (чтобы не пересчитывать коэффициенты каждый сэмпл).
    last_lp_cutoff: u32,
    /// Последняя применённая частота среза HP.
    last_hp_cutoff: u32,
}

impl<S: Source> LiveFilteredSource<S> {
    /// Создаёт источник с фильтрами.
    ///
    /// * `lp_cutoff` / `hp_cutoff` — разделяемые частоты среза (Hz, `0` = выключено).
    pub fn new(inner: S, lp_cutoff: Arc<AtomicU32>, hp_cutoff: Arc<AtomicU32>) -> Self {
        let sample_rate = inner.sample_rate().get() as f32;
        let channels = inner.channels();
        let lp_freq = lp_cutoff.load(Ordering::Relaxed) as f32;
        let hp_freq = hp_cutoff.load(Ordering::Relaxed) as f32;

        Self {
            inner,
            lp: BiquadFilter::new(FilterType::LowPass, sample_rate, lp_freq.max(20.0)),
            hp: BiquadFilter::new(FilterType::HighPass, sample_rate, hp_freq.max(20.0)),
            lp_cutoff,
            hp_cutoff,
            channels,
            lp_mix: 0.0,
            hp_mix: 0.0,
            ramp_step: 1.0 / (sample_rate * 0.01), // ~10 мс плавного включения/выключения
            last_lp_cutoff: lp_freq as u32,
            last_hp_cutoff: hp_freq as u32,
        }
    }
}

impl<S: Source> Iterator for LiveFilteredSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;

        // Подхватываем актуальные частоты среза (0 = выключено)
        let lp_freq = self.lp_cutoff.load(Ordering::Relaxed);
        let hp_freq = self.hp_cutoff.load(Ordering::Relaxed);

        // LP: пересчитываем коэффициенты только при реальном изменении частоты
        if lp_freq != self.last_lp_cutoff {
            self.lp.set_cutoff(lp_freq.max(20) as f32);
            self.last_lp_cutoff = lp_freq;
        }
        let lp_out = self.lp.process(sample);
        // LP на максимуме (20000 Гц) — полный байпас: не подмешиваем фильтр,
        // чтобы включение не сужало стерео и не меняло звук.
        let lp_target = if lp_freq > 0 && lp_freq < 20000 {
            1.0
        } else {
            0.0
        };
        self.lp_mix = ramp_toward(self.lp_mix, lp_target, self.ramp_step);
        let mut out = sample + (lp_out - sample) * self.lp_mix;

        // HP: аналогично
        if hp_freq != self.last_hp_cutoff {
            self.hp.set_cutoff(hp_freq.max(20) as f32);
            self.last_hp_cutoff = hp_freq;
        }
        let hp_out = self.hp.process(out);
        // HP на минимуме (20 Гц) — полный байпас.
        let hp_target = if hp_freq > 0 && hp_freq > 20 {
            1.0
        } else {
            0.0
        };
        self.hp_mix = ramp_toward(self.hp_mix, hp_target, self.ramp_step);
        out = out + (hp_out - out) * self.hp_mix;

        Some(out)
    }
}

impl<S: Source> Source for LiveFilteredSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> NonZeroU16 {
        self.channels
    }

    fn sample_rate(&self) -> NonZeroU32 {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    // Пробрасываем перемотку во внутренний источник.
    // Без этого rodio возвращает SeekError::NotSupported,
    // и движок сообщает об ошибке перемотки.
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        // После скачка сбрасываем задержки фильтров, чтобы не было щелчка/артефакта
        self.lp.reset();
        self.hp.reset();
        self.lp_mix = 0.0;
        self.hp_mix = 0.0;
        Ok(())
    }
}
