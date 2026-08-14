//! Модуль фильтров аудиосигнала (Low-Pass / High-Pass) с live-управлением.
//!
//! Фильтры читают частоту среза из `Arc<AtomicU32>` на каждом сэмпле,
//! поэтому их можно менять «на лету» без перезапуска источника.
//! Значение `0` в атомике означает «фильтр выключен».
//!
//! Используется готовый биквад из крейта `math-iir-fir` (`Biquad<f32>`)
//! с лёгким резонансом (Q = 1.0) для «музыкального» среза.
//! Включение/выключение фильтра сглаживается плавным ramp'ом (~10 мс),
//! чтобы не было щелчков при переключении.

use math_audio_iir_fir::{Biquad, BiquadFilterType};
use rodio::Source;
use rodio::source::SeekError;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Фактор добротности фильтра. 1.0 — лёгкий резонанс на срезе.
const FILTER_Q: f32 = 1.0; // 7071 default

/// Шаг плавного включения/выключения фильтра (в долях за сэмпл).
///
/// Фильтр не переключается мгновенно между «выкл» и «вкл», а плавно
/// подмешивается/убирается за ~10 мс. Это убирает щелчки при переключении.
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

    lp_left: Biquad<f32>,
    lp_right: Biquad<f32>,

    hp_left: Biquad<f32>,
    hp_right: Biquad<f32>,

    lp_cutoff: Arc<AtomicU32>,
    hp_cutoff: Arc<AtomicU32>,

    channels: NonZeroU16,
    channel_index: u16,

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

        let lp_initial = lp_freq.max(20.0);
        let hp_initial = hp_freq.max(20.0);

        Self {
            inner,

            lp_left: Biquad::new(
                BiquadFilterType::Lowpass,
                lp_initial,
                sample_rate,
                FILTER_Q,
                0.0,
            ),

            lp_right: Biquad::new(
                BiquadFilterType::Lowpass,
                lp_initial,
                sample_rate,
                FILTER_Q,
                0.0,
            ),

            hp_left: Biquad::new(
                BiquadFilterType::Highpass,
                hp_initial,
                sample_rate,
                FILTER_Q,
                0.0,
            ),

            hp_right: Biquad::new(
                BiquadFilterType::Highpass,
                hp_initial,
                sample_rate,
                FILTER_Q,
                0.0,
            ),

            lp_cutoff,
            hp_cutoff,

            channels,
            channel_index: 0,

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

        let channel = self.channel_index;
        self.channel_index = (self.channel_index + 1) % self.channels.get();

        // Подхватываем актуальные частоты среза (0 = выключено)
        let lp_freq = self.lp_cutoff.load(Ordering::Relaxed);
        let hp_freq = self.hp_cutoff.load(Ordering::Relaxed);

        // LP: пересчитываем коэффициенты только при реальном изменении частоты.
        // update_params не сбрасывает состояние фильтра — вращение ручки бесщёлковое.
        if lp_freq != self.last_lp_cutoff {
            let cutoff = lp_freq.max(20) as f32;

            self.lp_left.update_params(
                BiquadFilterType::Lowpass,
                cutoff,
                self.lp_left.srate,
                FILTER_Q,
                0.0,
            );

            self.lp_right.update_params(
                BiquadFilterType::Lowpass,
                cutoff,
                self.lp_right.srate,
                FILTER_Q,
                0.0,
            );

            self.last_lp_cutoff = lp_freq;
        }

        let lp_out = if channel == 0 {
            self.lp_left.process(sample)
        } else {
            self.lp_right.process(sample)
        };
        // LP ≥ 20000 Гц — полный байпас: не подмешиваем фильтр,
        // чтобы включение не сужало стерео и не меняло звук.
        let lp_target = if lp_freq > 0 && lp_freq < 20_000 {
            1.0
        } else {
            0.0
        };

        self.lp_mix = ramp_toward(self.lp_mix, lp_target, self.ramp_step);
        let mut out = sample + (lp_out - sample) * self.lp_mix;

        // HP: аналогично
        if hp_freq != self.last_hp_cutoff {
            let cutoff = hp_freq.max(20) as f32;

            self.hp_left.update_params(
                BiquadFilterType::Highpass,
                cutoff,
                self.hp_left.srate,
                FILTER_Q,
                0.0,
            );

            self.hp_right.update_params(
                BiquadFilterType::Highpass,
                cutoff,
                self.hp_right.srate,
                FILTER_Q,
                0.0,
            );

            self.last_hp_cutoff = hp_freq;
        }

        let hp_out = if channel == 0 {
            self.hp_left.process(out)
        } else {
            self.hp_right.process(out)
        };
        // HP ≤ 20 Гц — полный байпас.
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
        self.lp_left.reset();
        self.lp_right.reset();

        self.hp_left.reset();
        self.hp_right.reset();

        self.channel_index = 0;

        self.lp_mix = 0.0;
        self.hp_mix = 0.0;
        Ok(())
    }
}
