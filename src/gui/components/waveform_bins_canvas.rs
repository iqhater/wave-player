use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::core::use_drop;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::atomic::Ordering;
use std::time::Duration;

const WAVEFORM_JS: &str = r#"
    let data = await dioxus.recv();

    let targetPeaks = data.peaks || [];
    let progress = data.progress || 0;
    let duration = data.duration || 0;
    let reset = data.reset || false;

    let canvas = document.getElementById("waveform-canvas");

    if (!canvas) {
        return;
    }

    let ctx = canvas.getContext("2d");

    let width = canvas.clientWidth || 600;
    let height = canvas.clientHeight || 72;
    let dpr = window.devicePixelRatio || 1;

    if (
        canvas.width !== width * dpr ||
        canvas.height !== height * dpr
    ) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;

        // Установка transform вместо повторного ctx.scale().
        // Это предотвращает накопление масштабирования при resize.
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    window.waveDuration = duration;

    // Zoom и scroll являются общим состоянием для waveform,
    // beat grid, seek и loop-маркеров.
    if (reset || window.waveZoom === undefined) {
        window.waveZoom = 1.0;
        window.waveScroll = 0.0;
    }

    if (window.waveformAnimId) {
        cancelAnimationFrame(window.waveformAnimId);
        window.waveformAnimId = null;
    }

    if (
        !window.animatedPeaks ||
        window.animatedPeaks.length !== targetPeaks.length
    ) {
        window.animatedPeaks = new Array(targetPeaks.length).fill(0);
    }

    const LERP_FACTOR = 0.18;

    /*
     * Переводит положение внутри видимой области canvas
     * в абсолютное время трека.
     *
     * pct находится в диапазоне 0..1.
     */
    window.waveTimeAtViewportPct = function (pct) {
        const trackDuration = window.waveDuration || 0;
        const zoom = window.waveZoom || 1.0;
        const scroll = window.waveScroll || 0.0;

        if (trackDuration <= 0) {
            return 0;
        }

        const visibleDuration = trackDuration / zoom;
        const startTime =
            scroll * (trackDuration - visibleDuration);

        const normalizedPct = Math.max(0, Math.min(1, pct));

        return Math.max(
            0,
            Math.min(
                trackDuration,
                startTime + normalizedPct * visibleDuration
            )
        );
    };

    /*
     * Переводит абсолютное время трека в процент
     * относительно текущей видимой области canvas.
     */
    window.waveViewportPctAtTime = function (seconds) {
        const trackDuration = window.waveDuration || 0;
        const zoom = window.waveZoom || 1.0;
        const scroll = window.waveScroll || 0.0;

        if (trackDuration <= 0) {
            return 0;
        }

        const visibleDuration = trackDuration / zoom;
        const startTime =
            scroll * (trackDuration - visibleDuration);

        return (
            ((seconds - startTime) / visibleDuration) * 100
        );
    };

    /*
     * Обновляет CSS-переменные, используемые HTML-оверлеями:
     * - курсором;
     * - loop-маркерами.
     */
    function updateWaveViewportVariables() {
        const waveformCanvas =
            document.getElementById("waveform-canvas");

        if (!waveformCanvas) {
            return;
        }

        const waveformContainer =
            waveformCanvas.parentElement;

        if (!waveformContainer) {
            return;
        }

        const cursorTime =
            progress * (window.waveDuration || 0);

        const cursorPct =
            window.waveViewportPctAtTime(cursorTime);

        waveformContainer.style.setProperty(
            "--wave-cursor-pct",
            `${Math.max(0, Math.min(100, cursorPct))}%`
        );

        /*
         * LoopPlayer находится в том же waveform-контейнере.
         * Его значения доступны через data-loop-start/end.
         */
        const loopElement =
            document.querySelector(
                "[data-loop-start][data-loop-end]"
            );

        if (!loopElement) {
            return;
        }

        const loopStart =
            Number(loopElement.dataset.loopStart || 0);

        const loopEnd =
            Number(loopElement.dataset.loopEnd || 0);

        if (loopEnd <= loopStart) {
            return;
        }

        const loopStartPct =
            window.waveViewportPctAtTime(loopStart);

        const loopEndPct =
            window.waveViewportPctAtTime(loopEnd);

        loopElement.style.setProperty(
            "--wave-loop-start",
            `${Math.max(0, Math.min(100, loopStartPct))}%`
        );

        loopElement.style.setProperty(
            "--wave-loop-end",
            `${Math.max(0, Math.min(100, loopEndPct))}%`
        );
    }

    /*
     * Изменяет zoom относительно точки под курсором.
     *
     * mousePct находится в диапазоне 0..1.
     */
    window.zoomAt = function (mousePct, factor) {
        const zoom = window.waveZoom || 1.0;
        const scroll = window.waveScroll || 0.0;
        const totalDuration = window.waveDuration || 0;

        if (totalDuration <= 0) {
            return;
        }

        const normalizedMousePct =
            Math.max(0, Math.min(1, mousePct));

        const newZoom = Math.max(
            1.0,
            Math.min(80.0, zoom * factor)
        );

        const visibleOld = 1.0 / zoom;

        const startOld =
            scroll * (1.0 - visibleOld);

        /*
         * Абсолютная временная координата точки,
         * находящейся под курсором.
         */
        const anchor =
            startOld + normalizedMousePct * visibleOld;

        const visibleNew = 1.0 / newZoom;

        let startNew =
            anchor - normalizedMousePct * visibleNew;

        /*
         * Если zoom равен 1, виден весь трек,
         * поэтому scroll должен быть равен нулю.
         */
        let newScroll;

        if (visibleNew >= 1.0) {
            newScroll = 0;
        } else {
            newScroll =
                startNew / (1.0 - visibleNew);
        }

        newScroll = Math.max(
            0,
            Math.min(1, newScroll)
        );

        window.waveZoom = newZoom;
        window.waveScroll = newScroll;

        updateWaveViewportVariables();

        if (window.drawBeatGrid) {
            window.drawBeatGrid();
        }

        if (window.waveformAnimId) {
            cancelAnimationFrame(window.waveformAnimId);
        }

        window.waveformAnimId = requestAnimationFrame(animateFrame);
    };

    function animateFrame() {
        ctx.clearRect(0, 0, width, height);

        const totalBins = targetPeaks.length;

        if (totalBins === 0) {
            updateWaveViewportVariables();
            return;
        }

        const zoom = window.waveZoom || 1.0;
        const scroll = window.waveScroll || 0.0;

        /*
         * Viewport задаётся в нормализованных координатах
         * относительно полного waveform.
         */
        const visibleFraction = 1.0 / zoom;

        const startFraction =
            scroll * (1.0 - visibleFraction);

        const endFraction =
            startFraction + visibleFraction;

        /*
         * Дробные индексы важны: grid и waveform должны
         * использовать одну и ту же систему координат.
         */
        const startPosition =
            startFraction * totalBins;

        const endPosition =
            endFraction * totalBins;

        const visibleBins =
            Math.max(1, endPosition - startPosition);

        const step =
            width / visibleBins;

        let gap;

        if (step >= 4.0) {
            gap = 1.5;
        } else if (step >= 2.0) {
            gap = 1.0;
        } else {
            gap = 0.65;
        }

        const barWidth =
            Math.max(0.8, step - gap);

        const firstBin =
            Math.max(0, Math.floor(startPosition));

        const lastBin =
            Math.min(totalBins, Math.ceil(endPosition));

        let isAnimationDone = true;

        // Central horizontal line
        /* ctx.save();
        ctx.strokeStyle = "rgba(165, 196, 196, 0.18)";
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(0, height / 2);
        ctx.lineTo(width, height / 2);
        ctx.stroke();
        ctx.restore(); */

        for (let i = 0; i < totalBins; i++) {
            const target =
                Number(targetPeaks[i] || 0);

            const current =
                Number(window.animatedPeaks[i] || 0);

            const diff = target - current;

            if (Math.abs(diff) > 0.001) {
                window.animatedPeaks[i] =
                    current + diff * LERP_FACTOR;

                isAnimationDone = false;
            } else {
                window.animatedPeaks[i] = target;
            }
        }

        for (let i = firstBin; i < lastBin; i++) {
            const current =
                window.animatedPeaks[i] || 0;

            const normalizedPeak =
                Math.pow(Math.max(0.02, current), 0.85);

            const barHeight =
                Math.max(2.0, normalizedPeak * height);

            /*
             * Позиция учитывает дробный startPosition.
             * Поэтому waveform не скачет относительно сетки
             * при zoom и scroll.
             */
            const x =
                (i - startPosition) * step;

            const y =
                (height - barHeight) / 2;

            if ((i / totalBins) <= progress) {
                ctx.fillStyle =
                    "rgba(90, 122, 122, 1)";
            } else {
                ctx.fillStyle =
                    "rgba(196, 200, 192, 1)";
            }

            /* More contrast bars color
                if ((i / totalBins) <= progress) {
                    ctx.fillStyle = "rgba(110, 165, 165, 0.95)";
                } else {
                    ctx.fillStyle = "rgba(205, 210, 204, 0.78)";
                }
            */

            ctx.beginPath();

            if (ctx.roundRect && barWidth >= 3.0) {
                ctx.roundRect(
                    x,
                    y,
                    barWidth,
                    barHeight,
                    4
                );
            } else {
                ctx.rect(
                    x,
                    y,
                    barWidth,
                    barHeight
                );
            }

            ctx.fill();
        }

        updateWaveViewportVariables();

        if (!isAnimationDone) {
            window.waveformAnimId =
                requestAnimationFrame(animateFrame);
        } else {
            window.waveformAnimId = null;
        }
    }

    updateWaveViewportVariables();
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
            let initial_position = pos.load(Ordering::Relaxed);
            position_secs.set(initial_position as f32 / 1000.0);

            let mut last_position = initial_position;

            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;

                let current_position = pos.load(Ordering::Relaxed);

                if current_position != last_position {
                    last_position = current_position;
                    position_secs.set(current_position as f32 / 1000.0);
                }
            }
        }
        /* let pos = ctx.playback_progress.read().clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                position_secs.set(pos.load(Ordering::Relaxed) as f32 / 1000.0);
            }
        } */
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
                "duration": total_dur,
                "reset": do_reset
            }));
        });
    });

    // Clean up waveform animation resources on drop
    use_drop(|| {
        let _ = dioxus::document::eval(
            r#"
            if (window.waveformAnimId !== null) {
                cancelAnimationFrame(window.waveformAnimId);
                window.waveformAnimId = null;
            }

            window.waveViewportPctAtTime = undefined;
            window.waveTimeAtViewportPct = undefined;
            window.zoomAt = undefined;
            window.updateWaveViewportVariables = undefined;
            "#,
        );
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
