use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;

const BEAT_GRID_JS: &str = r#"
    let data = await dioxus.recv();
        window.beatGrid = {
        bpm: data.bpm,
        duration: data.duration,
        snap: data.snap,
        offset: Number(data.offset) || 0,
    };

    const MINP = 20;   // минимальный отступ между линиями в px

    // Общий расчёт шага сетки (используется и для отрисовки, и для магнита).
    // Делители такта: от 1/16 бита до 8 тактов.
    function gridStep(pps) {
        let mults = [1/64, 1/32, 1/16, 1/8, 1/4, 1/2, 1, 2, 4, 8]; // в долях такта
        let bar = 240 / window.beatGrid.bpm;
        for (let m of mults) {
            if (bar * m * pps >= MINP) return bar * m;
        }
        return bar * 8; // крайний случай — очень широкий трек
    }

    function firstGridLineAtOrAfter(time, step) {
        const offset =
            window.beatGrid.offset || 0;

        return (
            Math.ceil(
                (time - offset - 0.000001) / step
            ) * step +
            offset
        );
    }

    window.drawBeatGrid = function () {
        let canvas = document.getElementById("beat-grid-canvas");
        if (!canvas) return;
        let ctx = canvas.getContext("2d");
        let width = canvas.clientWidth || 400;
        let height = canvas.clientHeight || 72;
        let dpr = window.devicePixelRatio || 1;
        if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
            canvas.width = width * dpr; canvas.height = height * dpr; ctx.scale(dpr, dpr);
        }
        ctx.clearRect(0, 0, width, height);

        let bg = window.beatGrid;
        if (!bg || !bg.bpm || bg.bpm <= 0 || !bg.duration || bg.duration <= 0) return;

        let visible = bg.duration / (window.waveZoom || 1.0);
        let startTime = (window.waveScroll || 0.0) * (bg.duration - visible);
        let endTime = startTime + visible;
        let pps = width / visible;

        let beat = 60 / bg.bpm;
        let step = gridStep(pps);

        const firstGridLine =
            firstGridLineAtOrAfter(startTime, step);

        for (let t = firstGridLine; t <= endTime + 1e-6; t += step) {
            let pct = (t - startTime) / visible;
            if (pct < 0 || pct > 1) continue;
            let x = pct * width;

            const offset =
                window.beatGrid.offset || 0;

            const bar = 4 * beat;

            const barPosition =
                (t - offset) / bar;

            const beatPosition =
                (t - offset) / beat;

            const isBar =
                Math.abs(
                    barPosition - Math.round(barPosition)
                ) < 1e-4;

            const isBeat =
                !isBar &&
                Math.abs(
                    beatPosition - Math.round(beatPosition)
                ) < 1e-4;

            ctx.strokeStyle = isBar ? "rgba(255,255,255,0.5)"
                          : isBeat ? "rgba(255,255,255,0.25)"
                          : "rgba(255,255,255,0.10)";
            ctx.lineWidth = isBar ? 1.4 : 1;
            ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, height); ctx.stroke();
        }
    };

    // Превращает secs в ближайшую линию сетки. Уважает настройку snap.
    window.snapToGrid = function (seconds) {
        const grid = window.beatGrid;

        if (
            !grid ||
            !grid.snap ||
            !grid.bpm ||
            grid.bpm <= 0 ||
            !grid.duration ||
            grid.duration <= 0
        ) {
            return Math.max(
                0,
                Math.min(seconds, grid?.duration || seconds)
            );
        }

        const canvas =
            document.getElementById("beat-grid-canvas");

        const width =
            canvas?.clientWidth || 400;

        function getViewport() {
            const duration = window.beatGrid.duration;
            const zoom = window.waveZoom || 1.0;
            const scroll = window.waveScroll || 0.0;

            const visibleDuration = duration / zoom;

            const startTime =
                scroll * (duration - visibleDuration);

            const endTime =
                startTime + visibleDuration;

            return {
                startTime,
                endTime,
                visibleDuration,
            };
        }

        const viewport = getViewport();

        const pixelsPerSecond =
            width / viewport.visibleDuration;

        const step =
            gridStep(pixelsPerSecond);

        /*
         * Используем тот же первый grid line,
         * с которого начинается отрисовка.
         *
         * Это важно: snap и canvas теперь используют
         * абсолютно одинаковые координаты.
         */
        const firstGridLine =
            Math.ceil(
                (viewport.startTime - 0.000001) / step
            ) * step;

        /*
         * Проверяем ближайшие линии около точки клика.
         * Достаточно проверить floor и ceil.
         */
         const offset =
             window.beatGrid.offset || 0;

        const relativeIndex =
            (seconds - offset) / step;

        const lowerIndex =
            Math.floor(relativeIndex);

        const upperIndex =
            Math.ceil(relativeIndex);

        const lowerLine =
            firstGridLine + lowerIndex * step;

        const upperLine =
            firstGridLine + upperIndex * step;

        const snapped =
            Math.round(relativeIndex) * step +
            offset;

        return Math.max(
            0,
            Math.min(grid.duration, snapped)
        );
    };

    window.drawBeatGrid();
"#;

#[component]
pub fn BeatGrid(total_duration: ReadSignal<f32>, snap_enabled: ReadSignal<bool>) -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut bpm = use_signal(|| 0.0f32);
    let mut beat_offset = use_signal(|| 0.0f32);

    // Слушаем BPM из движка
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::BpmKeyDetected {
                        bpm: b,
                        beat_offset: offset,
                        ..
                    } => {
                        bpm.set(b);
                        beat_offset.set(offset);
                    }
                    PlayerEvent::Started { .. } => {
                        bpm.set(0.0);
                        beat_offset.set(0.0);
                    }
                    _ => {}
                }
            }
        }
    });

    // Перерисовка при изменении bpm/длительности/настройки
    use_effect(move || {
        let b = bpm();
        let d = total_duration();
        let s = snap_enabled();
        let offset = beat_offset();

        spawn(async move {
            let eval = dioxus::document::eval(BEAT_GRID_JS);
            let _ = eval.send(serde_json::json!({
                "bpm": b,
                "duration": d,
                "snap": s,
                "offset": offset,
            }));
        });
    });

    rsx! {
        canvas {
            id: "beat-grid-canvas",
            class: "absolute inset-0 w-full h-full pointer-events-none",
            style: "display: block; width: 100%; height: 100%;",
        }
    }
}
