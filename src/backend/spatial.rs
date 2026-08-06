// Mid, Side, Mono, Stereo, Width, Pan
//! Модуль пространственной обработки аудиосигнала (Spatial DSP Utility).
//! Поддерживает работу с 2 каналами (Стерео).

use rodio::Source;
use std::f32::consts::FRAC_PI_4;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Принудительное МОНО (Схлопывание стереосигнала).
///
/// Складывает левый и правый каналы и делит результат пополам.
/// Это предотвращает клиппинг (перегрузку) звука из-за сложения амплитуд.
#[inline(always)]
fn to_mono(left: f32, right: f32) -> (f32, f32) {
    let mono = (left + right) * 0.5;
    (mono, mono)
}

/// Разделение Стерео (Left/Right) на Mid и Side составляющие.
///
/// * `mid`  - Центральная (общая для обоих каналов) моно-информация.
/// * `side` - Боковая (разностная) информация, создающая ощущение пространства.
#[inline(always)]
fn to_mid_side(left: f32, right: f32) -> (f32, f32) {
    let mid = (left + right) * 0.5;
    let side = (left - right) * 0.5;
    (mid, side)
}

/// Сборка Mid и Side составляющих обратно в стандартное Стерео (Left/Right).
#[inline(always)]
fn from_mid_side(mid: f32, side: f32) -> (f32, f32) {
    let left = mid + side;
    let right = mid - side;
    (left, right)
}

/// Управление шириной стереобазы (Stereo Width) через Mid/Side матрицу.
///
/// # Аргументы
/// * `width` - Коэффициент ширины:
///   * `0.0` - Полное моно (боковая информация полностью стирается).
///   * `1.0` - Оригинальное, неизмененное стерео.
///   * `> 1.0` - Стереорасширение (усиление разности каналов делает звук «шире»).
///   * `0.0 .. 1.0` - Сужение стереобазы.
#[inline(always)]
fn apply_width(left: f32, right: f32, width: f32) -> (f32, f32) {
    // 1. Кодируем в Mid/Side
    let (mid, side) = to_mid_side(left, right);

    // 2. Масштабируем боковую составляющую
    let adjusted_side = side * width;

    // 3. Декодируем обратно в Left/Right
    from_mid_side(mid, adjusted_side)
}

/// Панорамирование постоянной мощности (Constant Power Pan).
///
/// В отличие от линейного баланса, этот алгоритм использует тригонометрические
/// функции (sin/cos). Благодаря этому при перемещении звука в центр
/// не происходит просадки по громкости (эффект «дыры в центре»).
///
/// # Аргументы
/// * `pan` - Положение панорамы от `-1.0` (полностью влево) до `1.0` (полностью вправо). `0.0` - центр.
#[inline(always)]
fn apply_pan(left: f32, right: f32, pan: f32) -> (f32, f32) {
    // Безопасно ограничиваем значение панорамы в рабочий диапазон
    let pan = pan.clamp(-1.0, 1.0);

    // Переводим диапазон [-1.0; 1.0] в радианы [0.0; PI/2]
    // В центральной точке (0.0) угол равен PI/4 (45 градусов)
    let pan_radian = (pan + 1.0) * FRAC_PI_4;

    // Вычисляем коэффициенты усиления (косинус для левого, синус для правого)
    let left_gain = pan_radian.sin();
    let right_gain = pan_radian.cos();

    (left * left_gain, right * right_gain)
}

/// Балансировка соотношения Mid и Side каналов напрямую (M/S Balance).
///
/// Позволяет отдельно управлять громкостью центрального образа (например, вокал, бас)
/// и пространственных эффектов (реверберация, дабл-треки гитар).
///
/// # Аргументы
/// * `mid_gain` - Коэффициент громкости для центрального канала (норма = `1.0`).
/// * `side_gain` - Коэффициент громкости для бокового канала (норма = `1.0`).
#[inline(always)]
fn apply_ms_gains(left: f32, right: f32, mid_gain: f32, side_gain: f32) -> (f32, f32) {
    let (mid, side) = to_mid_side(left, right);
    from_mid_side(mid * mid_gain, side * side_gain)
}

/// Шаг плавного перехода режима Mid/Side (в долях за сэмпл).
///
/// Переключение между стерео / Mid / Side выполняется не мгновенно,
/// а кроссфейдом за ~5 мс, чтобы не было щелчка.
#[inline(always)]
fn ramp_toward(current: f32, target: f32, step: f32) -> f32 {
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

/// Обёртка над rodio-источником с live-управляемой пространственной обработкой.
///
/// Параметры хранятся в `Arc<AtomicU32>` и читаются на каждом сэмпле,
/// поэтому эффект применяется «на лету» без перезапуска источника.
///
/// * `mono` — `0` = стерео, `1` = моно (схлопывание).
/// * `ms_mode` — `0` = стерео, `1` = изолировать Mid, `2` = изолировать Side.
/// * `width` — ширина стереобазы (f32::to_bits), `0.0..2.0`, норма `1.0`.
/// * `pan` — панорама (f32::to_bits), `-1.0..1.0`, центр `0.0`.
#[derive(Clone)]
pub struct LiveSpatialSource<S> {
    inner: S,
    ms_mode: Arc<AtomicU32>,
    width: Arc<AtomicU32>,
    pan: Arc<AtomicU32>,
    channels: NonZeroU16,
    pending: Option<f32>,
    /// Текущий режим Mid/Side (0 = стерео, 1 = Mid, 2 = Side).
    current_mode: u32,
    /// Доля кроссфейда к целевому режиму (0.0..=1.0).
    mode_mix: f32,
    /// Шаг кроссфейда за сэмпл (~5 мс).
    mode_ramp_step: f32,
}

impl<S: Source> LiveSpatialSource<S> {
    pub fn new(
        inner: S,
        ms_mode: Arc<AtomicU32>,
        width: Arc<AtomicU32>,
        pan: Arc<AtomicU32>,
    ) -> Self {
        let sample_rate = inner.sample_rate().get() as f32;
        Self {
            channels: inner.channels(),
            inner,
            ms_mode,
            width,
            pan,
            pending: None,
            current_mode: 0,
            mode_mix: 0.0,
            mode_ramp_step: 1.0 / (sample_rate * 0.005), // ~5 мс плавного перехода режима
        }
    }

    /// Обработка одной пары сэмплов (L/R).
    #[inline(always)]
    fn process_pair(&mut self, left: f32, right: f32) -> (f32, f32) {
        let ms_mode = self.ms_mode.load(Ordering::Relaxed);
        let width = f32::from_bits(self.width.load(Ordering::Relaxed));
        let pan = f32::from_bits(self.pan.load(Ordering::Relaxed));

        // 1. Ширина стереобазы (Mid/Side). Width = 0 → моно.
        let (mut l, mut r) = apply_width(left, right, width);

        // 2. Панорама (constant power)
        (l, r) = apply_pan(l, r, pan);

        // 3. Изоляция Mid / Side с плавным кроссфейдом между режимами.
        //    Считаем результат для текущего и целевого режима и смешиваем их,
        //    чтобы переключение не давало щелчка.
        let target_mode = ms_mode.clamp(0, 2);
        if target_mode != self.current_mode {
            self.mode_mix = ramp_toward(self.mode_mix, 1.0, self.mode_ramp_step);
            if self.mode_mix >= 1.0 {
                self.current_mode = target_mode;
                self.mode_mix = 0.0;
            }
        }

        // Результат в текущем режиме
        let (mut cur_l, mut cur_r) = (l, r);
        match self.current_mode {
            1 => {
                let (mid, _side) = to_mid_side(l, r);
                cur_l = mid;
                cur_r = mid;
            }
            2 => {
                let (_mid, side) = to_mid_side(l, r);
                cur_l = side;
                cur_r = side;
            }
            _ => {}
        }

        // Результат в целевом режиме (к которому идём)
        let (mut tgt_l, mut tgt_r) = (l, r);
        match target_mode {
            1 => {
                let (mid, _side) = to_mid_side(l, r);
                tgt_l = mid;
                tgt_r = mid;
            }
            2 => {
                let (_mid, side) = to_mid_side(l, r);
                tgt_l = side;
                tgt_r = side;
            }
            _ => {}
        }

        // Кроссфейд: пока идёт переход, смешиваем текущий и целевой режимы
        if self.mode_mix > 0.0 && self.mode_mix < 1.0 {
            let m = self.mode_mix;
            cur_l = cur_l + (tgt_l - cur_l) * m;
            cur_r = cur_r + (tgt_r - cur_r) * m;
            (cur_l, cur_r)
        } else {
            (cur_l, cur_r)
        }
    }
}

impl<S: Source> Iterator for LiveSpatialSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // Отдаём отложенный правый сэмпл предыдущей пары
        if let Some(p) = self.pending.take() {
            return Some(p);
        }

        // Не-стерео — пропускаем без обработки
        if self.channels.get() != 2 {
            return self.inner.next();
        }

        let left = self.inner.next()?;
        let right = match self.inner.next() {
            Some(r) => r,
            None => return Some(left), // нечётный сэмпл — пропускаем как есть
        };

        let (l, r) = self.process_pair(left, right);
        self.pending = Some(l);
        Some(r)
    }
}

impl<S: Source> Source for LiveSpatialSource<S> {
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

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.pending = None; // сбрасываем буфер пары при перемотке
        self.inner.try_seek(pos)
    }
}
