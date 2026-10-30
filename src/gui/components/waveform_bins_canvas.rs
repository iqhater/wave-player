use crate::PlayerContext;
use crate::gui::layout::LoopDragging;
use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::{PlayerEvent, WaveformEnvelope};
use dioxus::core::use_drop;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::atomic::Ordering;
use std::time::Duration;

const WAVEFORM_JS: &str = r#"
    let data = await dioxus.recv();

    let targetPeaks = data.peaks || [];
    //let progress = data.progress || 0;
    let duration = data.duration || 0;
    let reset = data.reset || false;
    const detail = data.detail || null;

    let canvas = document.getElementById("waveform-canvas");

    if (typeof data.progress === "number") {
        window.waveProgress = data.progress;
    }

    if (typeof window.waveProgress !== "number") {
        window.waveProgress = 0;
    }

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
            (window.waveProgress || 0) *
            (window.waveDuration || 0);

        const cursorPct =
            window.waveViewportPctAtTime(cursorTime);

        waveformContainer.style.setProperty(
            "--wave-cursor-pct",
            `${Math.max(0, Math.min(100, cursorPct))}%`
        );
    }

    window.updateWaveViewportVariables = updateWaveViewportVariables;

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
           // Math.min(80.0, zoom * factor)
            Math.min(1_000_000.0, zoom * factor)
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

    window.setWaveViewport = function (
        nextZoom,
        centerPercent
    ) {
        const parsedZoom = Number(nextZoom);
        const parsedCenter = Number(centerPercent);

        const zoom =
            Number.isFinite(parsedZoom)
                ? Math.max(1.0, parsedZoom)
                : 1.0;

        const center =
            Number.isFinite(parsedCenter)
                ? Math.max(0, Math.min(100, parsedCenter)) / 100
                : 0.5;

        const visibleFraction = 1.0 / zoom;
        const maxStart = Math.max(0, 1.0 - visibleFraction);

        const startFraction = Math.max(
            0,
            Math.min(
                maxStart,
                center - visibleFraction / 2.0
            )
        );

        window.waveZoom = zoom;

        window.waveScroll =
            maxStart > 0
                ? startFraction / maxStart
                : 0.0;

        updateWaveViewportVariables();

        if (window.drawBeatGrid) {
            window.drawBeatGrid();
        }

        if (window.waveformAnimId) {
                cancelAnimationFrame(window.waveformAnimId);
        }

        window.waveformAnimId =
                requestAnimationFrame(animateFrame);
    };

    window.panBy = function (deltaPx) {
        const canvas = document.getElementById("waveform-canvas");

        if (!canvas) {
            return;
        }

        const width = canvas.clientWidth || 1;
        const zoom = window.waveZoom || 1.0;
        const visibleFraction = 1.0 / zoom;
        const maxStart = 1.0 - visibleFraction;

        if (maxStart <= 0) {
            return;
        }

        const currentStart =
            (window.waveScroll || 0.0) * maxStart;

        const shift =
            (deltaPx / width) * visibleFraction;

        const nextStart =
            Math.max(0, Math.min(maxStart, currentStart + shift));

        window.waveScroll =
            nextStart / maxStart;

        updateWaveViewportVariables();

        if (window.drawBeatGrid) {
            window.drawBeatGrid();
        }

        if (window.waveformAnimId) {
            cancelAnimationFrame(window.waveformAnimId);
        }

        window.waveformAnimId =
            requestAnimationFrame(animateFrame);
    };

    window.handleWaveWheel = function (
        mousePct,
        deltaX,
        deltaY,
        shiftKey
    ) {
        const hasHorizontalDelta =
            Math.abs(deltaX) > Math.abs(deltaY) &&
            Math.abs(deltaX) > 0.01;

        const shouldPan =
            hasHorizontalDelta ||
            (shiftKey && Math.abs(deltaY) > 0.01);

        if (shouldPan) {
            const panDelta =
                hasHorizontalDelta ? deltaX : deltaY;

            window.panBy(panDelta);
        } else {
            const factor =
                deltaY < 0.0 ? 1.15 : 1.0 / 1.15;

            window.zoomAt(mousePct, factor);
        }

        return {
            zoom: window.waveZoom || 1.0,
            scroll: window.waveScroll || 0.0
        };
    };

    function drawDetailWaveform(detail, viewportStart, viewportSize) {
        const points = detail.points;
        const totalFrames = detail.totalFrames;

        const detailStart =
            detail.startFrame / totalFrames;

        const detailEnd =
            detail.endFrame / totalFrames;

        const pointSize =
            (detailEnd - detailStart) / points.length;

        for (let index = 0; index < points.length; index++) {
            const point = points[index];

            const pointPosition =
                detailStart + index * pointSize;

            const x =
                ((pointPosition - viewportStart) / viewportSize) * width;

            const nextX =
                ((pointPosition + pointSize - viewportStart) / viewportSize) * width;

            if (nextX < 0 || x > width) {
                continue;
            }

            const min = Math.max(-1, Math.min(1, point.min));
            const max = Math.max(-1, Math.min(1, point.max));

            const yTop =
                height / 2 - max * height / 2;

            const yBottom =
                height / 2 - min * height / 2;

            const barWidth =
                Math.max(1, nextX - x);

            const played =
                pointPosition <= (window.waveProgress || 0);

            ctx.fillStyle = played
                ? "rgba(90, 122, 122, 1)"
                : "rgba(196, 200, 192, 1)";

            ctx.fillRect(
                x,
                yTop,
                barWidth,
                Math.max(1, yBottom - yTop),
            );
        }
    }

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

        if (detail && detail.points && detail.points.length > 0) {
            drawDetailWaveform(detail, startFraction, visibleFraction);
            updateWaveViewportVariables();
            return;
        }

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

            if ((i / totalBins) <= (window.waveProgress || 0)) {
                ctx.fillStyle =
                    "rgba(90, 122, 122, 1)";
            } else {
                ctx.fillStyle =
                    "rgba(196, 200, 192, 1)";
            }

            /* More contrast bars color
                if ((i / totalBins) <= (window.waveProgress || 0)) {
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

    window.renderWaveform = function () {
        if (window.waveformAnimId) {
            return;
        }

        window.waveformAnimId =
            requestAnimationFrame(animateFrame);
    };

    if (
        typeof data.zoom === "number" &&
        typeof data.viewCenter === "number" &&
        window.setWaveViewport
    ) {
        window.setWaveViewport(
            data.zoom,
            data.viewCenter
        );
    }

    updateWaveViewportVariables();
    animateFrame();
"#;

const WAVEFORM_VIEWPORT_JS: &str = r#"
let data = await dioxus.recv();

if (window.setWaveViewport) {
    window.setWaveViewport(
        data.zoom,
        data.viewCenter
    );
}
"#;

const WAVEFORM_PROGRESS_JS: &str = r#"
let data = await dioxus.recv();

if (typeof data.progress === "number") {
    window.waveProgress = data.progress;

    if (window.updateWaveViewportVariables) {
        window.updateWaveViewportVariables();
    }

    if (window.renderWaveform) {
        window.renderWaveform();
    }
}
"#;

// Lightweight variant used while the loop region is being dragged: it only
// advances the playback cursor. The full canvas redraw is skipped so pointer
// handling keeps priority, but the cursor must never freeze.
const WAVEFORM_CURSOR_JS: &str = r#"
let data = await dioxus.recv();

if (typeof data.progress === "number") {
    window.waveProgress = data.progress;

    if (window.updateWaveViewportVariables) {
        window.updateWaveViewportVariables();
    }
}
"#;

#[derive(Clone, Debug)]
struct DetailWaveform {
    start_frame: usize,
    end_frame: usize,
    total_frames: usize,
    sample_rate: u32,
    points: Vec<WaveformEnvelope>,
}

#[component]
pub fn WaveformBinsCanvas(
    total_duration: ReadSignal<f32>,
    cursor_progress: ReadSignal<f32>,
    mut zoom: Signal<f32>,
    mut view_center: Signal<f32>,
) -> Element {
    let ctx = use_context::<PlayerContext>();
    let loop_dragging = use_context::<LoopDragging>().0;
    let mut position_secs = use_signal(|| 0.0);
    let num_bins = 500;
    let mut waveform_peaks = use_signal(|| vec![0.0_f32; num_bins]);

    let mut detail_waveform = use_signal(|| None::<DetailWaveform>);
    let mut active_detail_request = use_signal(|| None::<u64>);
    let mut next_detail_request = use_signal(|| 0_u64);

    // Сигнал сброса зума при загрузке нового трека
    let mut reset_zoom = use_signal(|| false);

    let mut detail_request_pending = use_signal(|| false);

    use_effect(move || {
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::RequestWaveformOverview);
    });

    // Фоновый слушатель событий от аудио-ядра
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started { .. } => {
                        waveform_peaks.set(vec![0.0f32; num_bins]);
                        detail_waveform.set(None);
                        active_detail_request.set(None);

                        // Сбрасываем зум в исходное состояние при запуске нового трека
                        reset_zoom.set(true);
                        zoom.set(1.0);
                        view_center.set(50.0);

                        detail_request_pending.set(false);
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
                    PlayerEvent::WaveformDetailReady {
                        request_id,
                        start_frame,
                        end_frame,
                        total_frames,
                        sample_rate,
                        points,
                    } => {
                        if active_detail_request() == Some(request_id) {
                            detail_waveform.set(Some(DetailWaveform {
                                start_frame,
                                end_frame,
                                total_frames,
                                sample_rate,
                                points,
                            }));

                            detail_request_pending.set(false);
                        }
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
    });

    use_effect(move || {
        if loop_dragging() {
            let _ = dioxus::document::eval(
                r#"
                if (window.waveformAnimId) {
                    cancelAnimationFrame(window.waveformAnimId);
                    window.waveformAnimId = null;
                }
                "#,
            );
        }
    });

    use_effect(move || {
        let dragging = loop_dragging();
        let current_pos = position_secs();
        let total_dur = total_duration();

        let progress = if total_dur > 0.0 {
            (current_pos / total_dur).clamp(0.0, 1.0)
        } else {
            0.0
        };

        spawn(async move {
            let script = if dragging {
                WAVEFORM_CURSOR_JS
            } else {
                WAVEFORM_PROGRESS_JS
            };
            let eval = dioxus::document::eval(script);

            let _ = eval.send(serde_json::json!({
                "progress": progress,
            }));
        });
    });

    // Перерисовка canvas при изменении пиков, позиции или зума
    use_effect(move || {
        if loop_dragging() {
            return;
        }

        let peaks = waveform_peaks.read().clone();
        let total_dur = total_duration();
        let do_reset = reset_zoom();

        if do_reset {
            reset_zoom.set(false);
        }

        let detail = detail_waveform().map(|data| {
            serde_json::json!({
                "startFrame": data.start_frame,
                "endFrame": data.end_frame,
                "totalFrames": data.total_frames,
                "sampleRate": data.sample_rate,
                "points": data.points,
            })
        });

        spawn(async move {
            let eval = dioxus::document::eval(WAVEFORM_JS);

            let _ = eval.send(serde_json::json!({
                "peaks": peaks,
                "duration": total_dur,
                "reset": do_reset,
                "detail": detail,
            }));
        });
    });

    const DETAIL_ZOOM_THRESHOLD: f32 = 8.0;

    use_effect(move || {
        let current_zoom = zoom();
        let current_view_center = view_center();

        spawn(async move {
            let eval = dioxus::document::eval(WAVEFORM_VIEWPORT_JS);

            let _ = eval.send(serde_json::json!({
                "zoom": current_zoom,
                "viewCenter": current_view_center,
            }));
        });
    });

    use_effect(move || {
        let duration = total_duration();
        let current_zoom = zoom();
        let center_percent = view_center();

        if duration <= 0.0 {
            return;
        }

        if current_zoom < DETAIL_ZOOM_THRESHOLD {
            detail_waveform.set(None);
            active_detail_request.set(None);
            detail_request_pending.set(false);
            return;
        }

        let visible_fraction = 1.0 / current_zoom;

        let center_fraction = (center_percent / 100.0).clamp(0.0, 1.0);

        // Load a wider range to avoid decoding on every follow movement.
        let request_fraction = (visible_fraction * 3.0).min(1.0);

        let start_fraction =
            (center_fraction - request_fraction / 2.0).clamp(0.0, 1.0 - request_fraction);

        let end_fraction = (start_fraction + request_fraction).min(1.0);

        if detail_request_pending() {
            return;
        }

        let request_id = next_detail_request.with_mut(|id| {
            *id = id.wrapping_add(1);
            *id
        });

        active_detail_request.set(Some(request_id));
        detail_request_pending.set(true);

        let start = Duration::from_secs_f32(duration * start_fraction);

        let end = Duration::from_secs_f32(duration * end_fraction);

        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::RequestWaveformDetail {
                request_id,
                start,
                end,
                points: 4096,
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
                let delta = e.delta().strip_units();
                let delta_x = delta.x;
                let delta_y = delta.y;
                let shift_key = e.modifiers().shift();
                let click_x = e.element_coordinates().x as f32;

                // Вся логика зума — в JS (синхронно, без гонок и без запроса ширины)
                let mut eval = dioxus::document::eval(
                    r#"
                    let data = await dioxus.recv();

                    const canvas =
                        document.getElementById("waveform-canvas");

                    if (!canvas || !window.handleWaveWheel) {
                        return;
                    }

                    const width =
                        canvas.clientWidth || 1;

                    const pct =
                        Math.max(0, Math.min(1, data.x / width));

                    const result =
                        window.handleWaveWheel(
                            pct,
                            data.deltaX,
                            data.deltaY,
                            data.shiftKey
                        );

                    dioxus.send(result);
                    "#
                );
                let _ = eval.send(serde_json::json!({
                    "x": click_x,
                    "deltaX": delta_x,
                    "deltaY": delta_y,
                    "shiftKey": shift_key,
                }));

                spawn(async move {
                    if let Ok(serde_json::Value::Object(data)) = eval.recv().await {
                        let new_zoom = data
                            .get("zoom")
                            .and_then(|value| value.as_f64())
                            .unwrap_or(1.0) as f32;

                        let scroll = data
                            .get("scroll")
                            .and_then(|value| value.as_f64())
                            .unwrap_or(0.0) as f32;

                        let visible_fraction = 1.0 / new_zoom.max(1.0);

                        let start_fraction =
                            scroll.clamp(0.0, 1.0) * (1.0 - visible_fraction);

                        let center_percent =
                            (start_fraction + visible_fraction / 2.0) * 100.0;

                        zoom.set(new_zoom);
                        view_center.set(center_percent.clamp(0.0, 100.0));
                    }
                });
            }
        }
    }
}
