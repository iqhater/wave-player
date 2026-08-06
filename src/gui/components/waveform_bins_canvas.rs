use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::atomic::Ordering;
use std::time::Duration;

// Группируем состояние зума в одну структуру для чистоты кода
#[derive(Clone, Copy, Debug, PartialEq)]
struct ZoomState {
    level: f32,  // Коэффициент масштабирования (1.0 = исходный, 80.0 = максимум)
    offset: f32, // Смещение просмотра трека (от 0.0 до 1.0)
}

// JS-код отрисовки волноформы на canvas. Вынесен в константу, чтобы не раздувать компонент.
const WAVEFORM_JS: &str = r#"
    let data = await dioxus.recv();
    let targetPeaks = data.peaks || [];
    let progress = data.progress || 0;
    let zoom = data.zoom || 1.0;
    let scroll = data.scroll || 0.0;

    let canvas = document.getElementById("waveform-canvas");
    if (!canvas) return;
    let ctx = canvas.getContext("2d");

    let width = canvas.clientWidth || 400;
    let height = canvas.clientHeight || 72;

    let dpr = window.devicePixelRatio || 1;
    if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;
        ctx.scale(dpr, dpr);
    }

    // Отменяем предыдущий кадр анимации, чтобы циклы не наслоились
    if (window.waveformAnimId) {
        cancelAnimationFrame(window.waveformAnimId);
    }

    // Пересоздаём массив только если длина реально изменилась,
    // сохраняя старые значения, чтобы не сбрасывать в плоскую линию
    if (!window.animatedPeaks || window.animatedPeaks.length !== targetPeaks.length) {
        window.animatedPeaks = new Array(targetPeaks.length).fill(0);
    }

    const LERP_FACTOR = 0.18;

    function animateFrame() {
        ctx.clearRect(0, 0, width, height);

        let totalLen = targetPeaks.length;
        if (totalLen === 0) return;

        // Окно зума: сколько баров помещается на холст (минимум 10 при макс. приближении)
        let visibleBarsCount = Math.max(10, Math.floor(totalLen / zoom));

        let maxScrollStartIndex = totalLen - visibleBarsCount;
        let startIdx = Math.max(0, Math.floor(scroll * maxScrollStartIndex));
        let endIdx = Math.min(totalLen, startIdx + visibleBarsCount);

        let step = width / (endIdx - startIdx);

        // Динамический зазор в зависимости от плотности баров
        let gap = 2.0;
        if (visibleBarsCount <= 100) gap = 3.0;
        else if (visibleBarsCount <= 300) gap = 1.5;
        else if (visibleBarsCount <= 600) gap = 1.0;
        else gap = 0.5;

        // Защита от отрицательной ширины: минимум 1 пиксель на бар
        let barWidth = Math.max(1.0, step - gap);

        let isAnimationDone = true;

        // ШАГ А: плавно обновляем высоты (LERP) для всего массива
        for (let i = 0; i < totalLen; i++) {
            let diff = targetPeaks[i] - window.animatedPeaks[i];
            if (Math.abs(diff) > 0.001) {
                window.animatedPeaks[i] += diff * LERP_FACTOR;
                isAnimationDone = false;
            } else {
                window.animatedPeaks[i] = targetPeaks[i];
            }
        }

        // ШАГ Б: рисуем только видимое окно
        for (let i = startIdx; i < endIdx; i++) {
            let current = window.animatedPeaks[i];
            let barHeight = Math.max(0.06, current) * height;

            let x = (i - startIdx) * step;
            let y = (height - barHeight) / 2;

            // Прогресс привязан к глобальному треку
            if ((i / totalLen) <= progress) {
                ctx.fillStyle = "rgba(90, 122, 122, 1)";
            } else {
                ctx.fillStyle = "rgba(196, 200, 192, 1)";
            }

            ctx.beginPath();
            // Скругляем только если бар достаточно широкий (>= 3px),
            // иначе обычный прямоугольник рендерится чётче
            if (ctx.roundRect && barWidth >= 3.0) {
                ctx.roundRect(x, y, barWidth, barHeight, 4);
            } else {
                ctx.rect(x, y, barWidth, barHeight);
            }
            ctx.fill();
        }

        if (!isAnimationDone) {
            window.waveformAnimId = requestAnimationFrame(animateFrame);
        }
    }

    animateFrame();
"#;

#[component]
pub fn WaveformBinsCanvas(
    total_duration: ReadSignal<f32>,
    cursor_progress: ReadSignal<f32>,
) -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut position_secs = use_signal(|| 0.0);
    let num_bins = 500;

    // Локальный стейт для хранения реальных высот столбиков
    let mut waveform_peaks = use_signal(|| vec![0.0f32; num_bins]);

    let mut zoom = use_signal(|| ZoomState {
        level: 1.0,
        offset: 0.0,
    });

    // Фоновый слушатель событий от аудио-ядра
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started { .. } => {
                        waveform_peaks.set(vec![0.0f32; num_bins]);

                        // Сбрасываем зум в исходное состояние при запуске нового трека
                        zoom.set(ZoomState {
                            level: 1.0,
                            offset: 0.0,
                        });
                    }
                    PlayerEvent::WaveformChunkReady {
                        chunk,
                        start_bin,
                        total_bins,
                    } => {
                        if waveform_peaks.read().len() != total_bins {
                            waveform_peaks.set(vec![0.0f32; total_bins]);
                        }
                        waveform_peaks.with_mut(|vec| {
                            for (i, peak_value) in chunk.into_iter().enumerate() {
                                let target_idx = start_bin + i;
                                if let Some(elem) = vec.get_mut(target_idx) {
                                    *elem = peak_value;
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
        }
    });

    // Периодически опрашиваем реальную позицию воспроизведения из атомика,
    // чтобы закрашивание волны обновлялось синхронно с курсором
    use_future(move || {
        let pos = ctx.playback_progress.read().clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                position_secs.set(pos.load(Ordering::Relaxed) as f32 / 1000.0);
            }
        }
    });

    // Перерисовка canvas при изменении пиков, позиции или зума
    use_effect(move || {
        let p_vec = waveform_peaks.read().clone();

        // Прогресс из реального времени воспроизведения
        let current_pos = position_secs();
        let total_dur = total_duration();
        let progress_ratio = if total_dur > 0.0 {
            (current_pos / total_dur).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let zoom_data = zoom();

        spawn(async move {
            let eval = dioxus::document::eval(WAVEFORM_JS);
            let _ = eval.send(serde_json::json!({
                "peaks": p_vec,
                "progress": progress_ratio,
                "zoom": zoom_data.level,
                "scroll": zoom_data.offset
            }));
        });
    });

    rsx! {
        canvas {
            id: "waveform-canvas",
            class: "w-full h-18 flex-shrink-0 pointer-events-auto",
            style: "display: block; width: 100%; height: 72px; background: transparent;",

            // Обработка вертикального колесика мыши (зум)
            onwheel: move |e| {
                let delta_y = e.delta().strip_units().y;
                let click_x = e.element_coordinates().x as f32;

                let current_state = zoom();
                let old_zoom = current_state.level;

                // delta_y < 0 — крутим от себя / приближение
                let mut new_zoom = if delta_y < 0.0 { old_zoom * 1.15 } else { old_zoom / 1.15 };
                new_zoom = new_zoom.clamp(1.0, 80.0);

                if new_zoom != old_zoom {
                    // Запрашиваем реальную ширину Canvas из WebView в момент скролла
                    let mut size_eval = dioxus::document::eval(
                        r#"
                        let canvas = document.getElementById("waveform-canvas");
                        dioxus.send(canvas ? canvas.clientWidth : 0);
                        "#
                    );

                    spawn(async move {
                        if let Ok(serde_json::Value::Number(w_val)) = size_eval.recv().await {
                            if let Some(canvas_width) = w_val.as_f64() {
                                let canvas_width = canvas_width as f32;

                                if canvas_width > 0.0 {
                                    // Процент мыши относительно реальной ширины Canvas
                                    let mouse_screen_pct = (click_x / canvas_width).clamp(0.0, 1.0);

                                    // Глобальная точка трека под курсором при текущем зуме
                                    let global_target_pct = current_state.offset + (mouse_screen_pct / old_zoom);

                                    // Новый скролл так, чтобы эта точка осталась под курсором
                                    let new_scroll = (global_target_pct - (mouse_screen_pct / new_zoom)).clamp(0.0, 1.0);

                                    zoom.set(ZoomState {
                                        level: new_zoom,
                                        offset: new_scroll,
                                    });
                                }
                            }
                        }
                    });
                }
            }
        }
    }
}
