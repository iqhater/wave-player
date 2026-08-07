use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::atomic::Ordering;
use std::time::Duration;

const WAVEFORM_JS: &str = r#"
    let data = await dioxus.recv();
    let targetPeaks = data.peaks || [];
    let progress = data.progress || 0;
    let reset = data.reset || false;

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

    // Зум/скролл живут здесь (в JS), чтобы якорение было точным и без гонок
    if (reset || window.waveZoom === undefined) {
        window.waveZoom = 1.0;
        window.waveScroll = 0.0;
    }

    if (window.waveformAnimId) {
        cancelAnimationFrame(window.waveformAnimId);
    }

    if (!window.animatedPeaks || window.animatedPeaks.length !== targetPeaks.length) {
        window.animatedPeaks = new Array(targetPeaks.length).fill(0);
    }

    const LERP_FACTOR = 0.18;

    // Якорный зум: точка под курсором остаётся на месте
    window.zoomAt = function(mousePct, factor) {
        let zoom = window.waveZoom;
        let scroll = window.waveScroll;
        let totalLen = targetPeaks.length;
        if (totalLen === 0) return;

        let newZoom = Math.max(1.0, Math.min(80.0, zoom * factor));

        // Точная видимая доля с учётом клампа в рендерере

        let visibleOld = 1.0 / zoom;

        let startOld = scroll * (1.0 - visibleOld);   // непрерывно
        let anchor = startOld + mousePct * visibleOld;


        let visibleNew = 1.0 / newZoom;
        let startNew = anchor - mousePct * visibleNew;

        // Обратно в долю прокручиваемого диапазона.
        // При visibleNew >= 1.0 (zoom = 1.0) весь трек виден — скролл всегда 0.
        let newScroll;
        if (visibleNew >= 1.0) {
            newScroll = 0;
        } else {
            newScroll = startNew / (1.0 - visibleNew);
        }
        newScroll = Math.max(0, Math.min(1, newScroll));

        window.waveZoom = newZoom;
        window.waveScroll = newScroll;
        window.waveformAnimId = requestAnimationFrame(animateFrame);
    };

    function animateFrame() {
        ctx.clearRect(0, 0, width, height);

        let totalLen = targetPeaks.length;
        if (totalLen === 0) return;

        let zoom = window.waveZoom;
        let scroll = window.waveScroll;

        let visibleBarsCount = Math.max(10, Math.floor(totalLen / zoom));

        let maxScrollStartIndex = totalLen - visibleBarsCount;
        let startIdx = Math.max(0, Math.floor(scroll * maxScrollStartIndex));
        let endIdx = Math.min(totalLen, startIdx + visibleBarsCount);

        let step = width / (endIdx - startIdx);

        let gap = 2.0;
        if (visibleBarsCount <= 100) gap = 3.0;
        else if (visibleBarsCount <= 300) gap = 1.5;
        else if (visibleBarsCount <= 600) gap = 1.0;
        else gap = 0.5;

        let barWidth = Math.max(1.0, step - gap);

        let isAnimationDone = true;

        for (let i = 0; i < totalLen; i++) {
            let diff = targetPeaks[i] - window.animatedPeaks[i];
            if (Math.abs(diff) > 0.001) {
                window.animatedPeaks[i] += diff * LERP_FACTOR;
                isAnimationDone = false;
            } else {
                window.animatedPeaks[i] = targetPeaks[i];
            }
        }

        for (let i = startIdx; i < endIdx; i++) {
            let current = window.animatedPeaks[i];
            let barHeight = Math.max(0.06, current) * height;

            let x = (i - startIdx) * step;
            let y = (height - barHeight) / 2;

            if ((i / totalLen) <= progress) {
                ctx.fillStyle = "rgba(90, 122, 122, 1)";
            } else {
                ctx.fillStyle = "rgba(196, 200, 192, 1)";
            }

            ctx.beginPath();
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

    // Сигнал сброса зума при загрузке нового трека
    let mut reset_zoom = use_signal(|| false);

    // Фоновый слушатель событий от аудио-ядра
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started { .. } => {
                        waveform_peaks.set(vec![0.0f32; num_bins]);

                        // Сбрасываем зум в исходное состояние при запуске нового трека
                        reset_zoom.set(true);
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

        // let zoom_data = zoom();
        let do_reset = reset_zoom();
        if do_reset {
            reset_zoom.set(false);
        }

        spawn(async move {
            let eval = dioxus::document::eval(WAVEFORM_JS);
            let _ = eval.send(serde_json::json!({
                "peaks": p_vec,
                "progress": progress_ratio,
                "reset": do_reset
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

                // factor: delta_y < 0 — приближение
                let factor = if delta_y < 0.0 { 1.15 } else { 1.0 / 1.15 };

                // Вся логика зума — в JS (синхронно, без гонок и без запроса ширины)
                let eval = dioxus::document::eval(
                    r#"
                    let data = await dioxus.recv();
                    let canvas = document.getElementById("waveform-canvas");
                    if (!canvas || !window.zoomAt) return;
                    let w = canvas.clientWidth || 1;
                    let pct = Math.max(0, Math.min(1, data.x / w));
                    window.zoomAt(pct, data.factor);
                    "#
                );
                let _ = eval.send(serde_json::json!({
                    "x": click_x,
                    "factor": factor
                }));
            }
        }
    }
}
