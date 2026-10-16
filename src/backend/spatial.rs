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

/// Балансировка левого и правого каналов
/// с constant-power коэффициентами.
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
    let left_gain = pan_radian.cos();
    let right_gain = pan_radian.sin();

    (left * left_gain, right * right_gain)
}

#[inline(always)]
fn apply_monitor_mode(left: f32, right: f32, mode: u32) -> (f32, f32) {
    match mode {
        // Mid: оставляем центральную составляющую
        // и выводим её одинаково в оба канала.
        1 => {
            let (mid, _) = to_mid_side(left, right);

            from_mid_side(mid, 0.0)
        }

        // Side: сохраняем противофазное L/R-представление.
        2 => {
            let (_, side) = to_mid_side(left, right);

            from_mid_side(0.0, side)
        }

        // Stereo
        _ => (left, right),
    }
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
/// * `ms_mode` — `0` = стерео, `1` = изолировать Mid,
///   `2` = изолировать Side.
/// * `width` — ширина стереобазы:
///   `0.0` = mono, `1.0` = исходная ширина,
///   больше `1.0` = расширение.
/// * `pan` — баланс от `-1.0` влево до `1.0` вправо.
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

        // 1. Изменяем ширину стереобазы.
        let (l, r) = apply_width(left, right, width);

        // 2. Определяем целевой режим.
        let target_mode = ms_mode.clamp(0, 2);

        if target_mode != self.current_mode {
            self.mode_mix = ramp_toward(self.mode_mix, 1.0, self.mode_ramp_step);

            if self.mode_mix >= 1.0 {
                self.current_mode = target_mode;

                self.mode_mix = 0.0;
            }
        }

        // 3. Результат текущего режима.
        let current = apply_monitor_mode(l, r, self.current_mode);

        // 4. Результат целевого режима.
        let target = apply_monitor_mode(l, r, target_mode);

        // 5. Плавный переход между режимами.
        let (mode_l, mode_r) = if self.mode_mix > 0.0 && self.mode_mix < 1.0 {
            let mix = self.mode_mix;

            (
                current.0 + (target.0 - current.0) * mix,
                current.1 + (target.1 - current.1) * mix,
            )
        } else {
            current
        };

        // 6. Pan применяется последним.
        apply_pan(mode_l, mode_r, pan)
    }
}

impl<S: Source> Iterator for LiveSpatialSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // Возвращаем правый канал предыдущей пары.
        if let Some(sample) = self.pending.take() {
            return Some(sample);
        }

        // Для mono и других нестерео-источников
        // сохраняем исходное поведение.
        if self.channels.get() != 2 {
            return self.inner.next();
        }

        let left = self.inner.next()?;

        let right = match self.inner.next() {
            Some(sample) => sample,
            None => return Some(left),
        };

        // let logical_left = source_first;
        // let logical_right = source_second;

        let (processed_left, processed_right) = self.process_pair(left, right);

        /*
         * Наружу всегда отдаём канонический порядок L/R.
         */
        self.pending = Some(processed_right);
        Some(processed_left)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    const EPSILON: f32 = 1e-5;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    fn assert_pair_close(actual: (f32, f32), expected: (f32, f32)) {
        assert_close(actual.0, expected.0);
        assert_close(actual.1, expected.1);
    }

    fn spatial_source<S: Source>(
        source: S,
        mode: u32,
        width: f32,
        pan: f32,
    ) -> LiveSpatialSource<S> {
        LiveSpatialSource::new(
            source,
            Arc::new(AtomicU32::new(mode)),
            Arc::new(AtomicU32::new(width.to_bits())),
            Arc::new(AtomicU32::new(pan.to_bits())),
        )
    }

    fn settled_spatial_source<S: Source>(
        source: S,
        mode: u32,
        width: f32,
        pan: f32,
    ) -> LiveSpatialSource<S> {
        let mut spatial = spatial_source(source, mode, width, pan);

        // В production transition занимает 5 мс.
        // В unit-тесте проверяем конечное состояние режима.
        spatial.mode_ramp_step = 1.0;

        spatial
    }

    #[derive(Clone)]
    struct TestStereoSource {
        samples: Vec<f32>,
        index: usize,
    }

    impl Iterator for TestStereoSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            let sample = self.samples.get(self.index).copied()?;

            self.index += 1;

            Some(sample)
        }
    }

    impl Source for TestStereoSource {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.samples.len().saturating_sub(self.index))
        }

        fn channels(&self) -> NonZeroU16 {
            NonZeroU16::new(2).unwrap()
        }

        fn sample_rate(&self) -> NonZeroU32 {
            NonZeroU32::new(48_000).unwrap()
        }

        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    fn test_source(samples: &[f32]) -> TestStereoSource {
        TestStereoSource {
            samples: samples.to_vec(),
            index: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Mid/Side conversion
    // -----------------------------------------------------------------------

    #[test]
    fn mid_side_round_trip_preserves_channels() {
        let input = (0.75, -0.25);

        let (mid, side) = to_mid_side(input.0, input.1);

        let output = from_mid_side(mid, side);

        assert_pair_close(output, input);
    }

    #[test]
    fn equal_channels_have_zero_side() {
        let (mid, side) = to_mid_side(0.5, 0.5);

        assert_close(mid, 0.5);
        assert_close(side, 0.0);
    }

    #[test]
    fn opposite_channels_have_zero_mid() {
        let (mid, side) = to_mid_side(1.0, -1.0);

        assert_close(mid, 0.0);
        assert_close(side, 1.0);
    }

    // -----------------------------------------------------------------------
    // Stereo width
    // -----------------------------------------------------------------------

    #[test]
    fn width_zero_produces_equal_channels() {
        let output = apply_width(1.0, 0.0, 0.0);

        assert_pair_close(output, (0.5, 0.5));
    }

    #[test]
    fn width_one_preserves_stereo() {
        let input = (0.75, -0.25);

        let output = apply_width(input.0, input.1, 1.0);

        assert_pair_close(output, input);
    }

    #[test]
    fn width_scales_side_component() {
        let input = (1.0, 0.0);

        let output = apply_width(input.0, input.1, 2.0);

        assert_pair_close(output, (1.5, -0.5));
    }

    // -----------------------------------------------------------------------
    // Pan
    // -----------------------------------------------------------------------

    #[test]
    fn pan_center_uses_equal_power_gains() {
        let output = apply_pan(1.0, 1.0, 0.0);

        let expected = FRAC_PI_4.cos();

        assert_pair_close(output, (expected, expected));
    }

    #[test]
    fn pan_left_removes_right_channel() {
        let output = apply_pan(1.0, 1.0, -1.0);

        assert_pair_close(output, (1.0, 0.0));
    }

    #[test]
    fn pan_right_removes_left_channel() {
        let output = apply_pan(1.0, 1.0, 1.0);

        assert_pair_close(output, (0.0, 1.0));
    }

    #[test]
    fn pan_is_clamped_to_valid_range() {
        assert_pair_close(apply_pan(1.0, 1.0, -2.0), apply_pan(1.0, 1.0, -1.0));

        assert_pair_close(apply_pan(1.0, 1.0, 2.0), apply_pan(1.0, 1.0, 1.0));
    }

    // -----------------------------------------------------------------------
    // Monitor modes
    // -----------------------------------------------------------------------

    #[test]
    fn stereo_mode_preserves_channels() {
        let input = (0.75, -0.25);

        let output = apply_monitor_mode(input.0, input.1, 0);

        assert_pair_close(output, input);
    }

    #[test]
    fn mid_mode_outputs_equal_channels() {
        let output = apply_monitor_mode(1.0, -0.5, 1);

        assert_close(output.0, output.1);
        assert_close(output.0, 0.25);
    }

    #[test]
    fn side_mode_outputs_opposite_channels() {
        let output = apply_monitor_mode(1.0, -0.5, 2);

        assert_pair_close(output, (0.75, -0.75));
    }

    #[test]
    fn invalid_monitor_mode_falls_back_to_stereo() {
        let input = (0.75, -0.25);

        assert_pair_close(apply_monitor_mode(input.0, input.1, 3), input);

        assert_pair_close(apply_monitor_mode(input.0, input.1, u32::MAX), input);
    }

    // -----------------------------------------------------------------------
    // Mode transition helper
    // -----------------------------------------------------------------------

    #[test]
    fn ramp_moves_up_without_overshooting() {
        assert_close(ramp_toward(0.0, 1.0, 0.25), 0.25);

        assert_close(ramp_toward(0.9, 1.0, 0.25), 1.0);
    }

    #[test]
    fn ramp_moves_down_without_overshooting() {
        assert_close(ramp_toward(1.0, 0.0, 0.25), 0.75);

        assert_close(ramp_toward(0.1, 0.0, 0.25), 0.0);
    }

    #[test]
    fn ramp_stays_at_target() {
        assert_close(ramp_toward(0.5, 0.5, 0.25), 0.5);
    }

    // -----------------------------------------------------------------------
    // LiveSpatialSource integration
    // -----------------------------------------------------------------------

    #[test]
    fn live_source_preserves_current_output_channel_order() {
        let source = test_source(&[0.8, 0.2, 0.7, 0.3]);

        let mut spatial = settled_spatial_source(source, 0, 1.0, 0.0);

        let gain = FRAC_PI_4.cos();

        let output = [
            spatial.next().unwrap(),
            spatial.next().unwrap(),
            spatial.next().unwrap(),
            spatial.next().unwrap(),
        ];

        /*
         * process_pair returns:
         * (left * gain, right * gain)
         *
         * Current next() intentionally exposes:
         * (processed_right, processed_left)
         */
        assert_close(output[0], 0.8 * gain);
        assert_close(output[1], 0.2 * gain);
        assert_close(output[2], 0.7 * gain);
        assert_close(output[3], 0.3 * gain);
    }

    #[test]
    fn live_source_pan_left_uses_expected_output_order() {
        let source = test_source(&[1.0, 1.0]);

        let mut spatial = spatial_source(source, 0, 1.0, -1.0);

        let first = spatial.next().unwrap();
        let second = spatial.next().unwrap();

        /*
         * apply_pan(-1) produces (1, 0).
         * next() exposes the current device order,
         * therefore the output pair is (0, 1).
         */
        assert_close(first, 1.0);
        assert_close(second, 0.0);
    }

    #[test]
    fn live_source_pan_right_uses_expected_output_order() {
        let source = test_source(&[1.0, 1.0]);

        let mut spatial = spatial_source(source, 0, 1.0, 1.0);

        let first = spatial.next().unwrap();
        let second = spatial.next().unwrap();

        /*
         * apply_pan(1) produces (0, 1).
         * next() exposes the current device order,
         * therefore the output pair is (1, 0).
         */
        assert_close(first, 0.0);
        assert_close(second, 1.0);
    }

    #[test]
    fn live_source_mid_mode_outputs_equal_channels() {
        let source = test_source(&[1.0, -0.5]);

        let mut spatial = settled_spatial_source(source, 1, 1.0, 0.0);

        let first = spatial.next().unwrap();

        let second = spatial.next().unwrap();

        let expected = 0.25 * FRAC_PI_4.cos();

        assert_close(first, expected);
        assert_close(second, expected);
    }

    #[test]
    fn live_source_side_mode_outputs_opposite_channels() {
        let source = test_source(&[1.0, -0.5]);

        let mut spatial = settled_spatial_source(source, 2, 1.0, 0.0);

        let first = spatial.next().unwrap();
        let second = spatial.next().unwrap();

        let expected = 0.75 * FRAC_PI_4.cos();

        /*
         * apply_monitor_mode(Side):
         * (0.75, -0.75)
         *
         * apply_pan(Center):
         * (0.75 * gain, -0.75 * gain)
         *
         * next() currently returns:
         * (processed_right, processed_left)
         */
        assert_close(first, expected);
        assert_close(second, -expected);
    }
}
