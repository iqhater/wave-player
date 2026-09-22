use crate::PlayerContext;
use crate::shared::event_types::{BeatGridData, PlayerEvent};
// use dioxus::core::use_drop;
use dioxus::prelude::*;
use futures_util::StreamExt;

const BEAT_GRID_JS: &str = r#"
    let data = await dioxus.recv();
        window.beatGrid = {
        bpm: data.bpm,
        duration: data.duration,
        snap: data.snap,
        offset: Number(data.offset) || 0,
        beats: data.beats || [],
        bars: data.bars || [],
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

        const beats = Array.isArray(bg.beats)
            ? bg.beats
            : [];

        const bars = Array.isArray(bg.bars)
            ? bg.bars
            : [];

            const beatDuration =
                60.0 / bg.bpm;

            const beatPhase =
                Number.isFinite(Number(bg.beats?.[0]))
                    ? Number(bg.beats[0])
                    : Number(bg.offset) || 0;

            const barPhase =
                Number.isFinite(Number(bg.bars?.[0]))
                    ? Number(bg.bars[0])
                    : beatPhase;

            const barDuration =
                beatDuration * 4.0;

            let gridInterval = beatDuration;
            let gridPhase = beatPhase;
            let gridColor = "rgba(255,255,255,0.22)";
            let gridWidth = 1.0;

            // At zoom out, use bar lines instead of individual beats.
            if (gridInterval * pps < MINP * 1.5) {
                gridInterval = barDuration;
                gridPhase = barPhase;
                gridColor = "rgba(255,255,255,0.45)";
                gridWidth = 1.4;
            }

            // If bars are still too dense, skip every second/fourth bar.
            while (gridInterval * pps < MINP * 1.5) {
                gridInterval *= 2.0;
            }

            const firstIndex = Math.ceil(
                (startTime - gridPhase) / gridInterval - 0.000001
            );

            const lastIndex = Math.floor(
                (endTime - gridPhase) / gridInterval + 0.000001
            );

            for (
                let index = firstIndex;
                index <= lastIndex;
                index++
            ) {
                const time =
                    gridPhase + index * gridInterval;

                const pct =
                    (time - startTime) / visible;

                const x =
                    pct * width;

                ctx.strokeStyle = gridColor;
                ctx.lineWidth = gridWidth;

                ctx.beginPath();
                ctx.moveTo(x, 0);
                ctx.lineTo(x, height);
                ctx.stroke();
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

        const beatDuration =
            60.0 / grid.bpm;

        const beatPhase =
            Number.isFinite(Number(grid.beats?.[0]))
                ? Number(grid.beats[0])
                : Number(grid.offset) || 0;

        const beatIndex =
            Math.round(
                (seconds - beatPhase) / beatDuration
            );

        const snapped =
            beatPhase + beatIndex * beatDuration;

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
    let mut beat_grid = use_signal(|| None::<BeatGridData>);

    // Слушаем BPM из движка
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::BpmKeyDetected {
                        bpm: b,
                        beat_grid: grid,
                        ..
                    } => {
                        bpm.set(b);
                        beat_offset.set(grid.offset);
                        beat_grid.set(Some(grid));
                    }
                    PlayerEvent::Started { .. } => {
                        bpm.set(0.0);
                        beat_offset.set(0.0);
                        beat_grid.set(None);
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
            let grid = beat_grid();

            let _ = eval.send(serde_json::json!({
                "bpm": b,
                "duration": d,
                "snap": s,
                "offset": offset,
                "beats": grid.as_ref().map(|grid| &grid.beats),
                "bars": grid.as_ref().map(|grid| &grid.bars),
            }));
        });
    });

    // Clean up beat grid animation resources on drop
    /* use_drop(|| {
        let _ = dioxus::document::eval(
            r#"
            if (window.beatGridAnimId !== null) {
                cancelAnimationFrame(window.beatGridAnimId);
                window.beatGridAnimId = null;
            }

            window.drawBeatGrid = undefined;
            "#,
        );
    }); */

    rsx! {
        canvas {
            id: "beat-grid-canvas",
            class: "absolute inset-0 w-full h-full pointer-events-none",
            style: "display: block; width: 100%; height: 100%;",
        }
    }
}
