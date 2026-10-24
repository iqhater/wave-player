use crate::gui::layout::PlaybackUIState;
use crate::gui::theme::{Theme, use_theme};
use crate::shared::event_types::BeatGridData;
use dioxus::prelude::*;

fn is_valid_tempo(tempo: f32) -> bool {
    tempo.is_finite() && tempo > 0.0
}

/// Manual tempo wins; otherwise the rounded BPM shown in the UI is used.
pub(crate) fn resolved_grid_tempo(bpm: f32, manual_tempo: Option<f32>) -> Option<f32> {
    manual_tempo
        .filter(|tempo| is_valid_tempo(*tempo))
        .or_else(|| is_valid_tempo(bpm).then_some(bpm))
}

/// Shift that keeps `anchor` on a beat line for the given tempo.
pub(crate) fn grid_shift_for_anchor(grid: &BeatGridData, tempo: f32, anchor: Option<f32>) -> f32 {
    let Some(anchor) = anchor.filter(|anchor| anchor.is_finite()) else {
        return 0.0;
    };

    let beat_duration = 60.0 / tempo;
    let base_phase = grid.beats.first().copied().unwrap_or(grid.offset);
    let nearest = base_phase + ((anchor - base_phase) / beat_duration).round() * beat_duration;

    anchor - nearest
}

const BEAT_GRID_JS: &str = r#"
    let data = await dioxus.recv();
        window.beatGrid = {
        bpm: data.bpm,
        tempo: Number(data.tempo) || Number(data.bpm) || 0,
        duration: data.duration,
        snap: data.snap,
        offset: Number(data.offset) || 0,
        shift: Number(data.shift) || 0,
        beats: data.beats || [],
        bars: data.bars || [],
        theme: data.theme || "light",
    };

    const MINP = 20;   // минимальный отступ между линиями в px

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
        if (!bg || !bg.tempo || bg.tempo <= 0 || !bg.duration || bg.duration <= 0) return;

        let visible = bg.duration / (window.waveZoom || 1.0);
        let startTime = (window.waveScroll || 0.0) * (bg.duration - visible);
        let endTime = startTime + visible;
        let pps = width / visible;

            const beatDuration =
                60.0 / bg.tempo;

            // Detected beat times can be sparse, so the grid stays uniform
            // and only takes its phase from the analysis.
            const baseBeatPhase =
                Array.isArray(bg.beats) && Number.isFinite(Number(bg.beats[0]))
                    ? Number(bg.beats[0])
                    : Number(bg.offset) || 0;

            const baseBarPhase =
                Array.isArray(bg.bars) && Number.isFinite(Number(bg.bars[0]))
                    ? Number(bg.bars[0])
                    : baseBeatPhase;

            const beatPhase = baseBeatPhase + bg.shift;
            const barPhase = baseBarPhase + bg.shift;

            const barDuration =
                beatDuration * 4.0;

            const isDark = bg.theme === "dark";

            // На светлой теме используем тёмный on-surface цвет,
            // иначе белые линии полностью теряются на светлом фоне.
            const beatColor = isDark
                ? "rgba(255,255,255,0.22)"
                : "rgba(27,28,28,0.26)";

            const barColor = isDark
                ? "rgba(255,255,255,0.45)"
                : "rgba(27,28,28,0.50)";

            let gridInterval = beatDuration;
            let gridPhase = beatPhase;
            let gridColor = beatColor;
            let gridWidth = 1.0;

            // At zoom out, use bar lines instead of individual beats.
            if (gridInterval * pps < MINP * 1.5) {
                gridInterval = barDuration;
                gridPhase = barPhase;
                gridColor = barColor;
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
            !grid.tempo ||
            grid.tempo <= 0 ||
            !grid.duration ||
            grid.duration <= 0
        ) {
            return Math.max(
                0,
                Math.min(seconds, grid?.duration || seconds)
            );
        }

        const beatDuration =
            60.0 / grid.tempo;

        const beatPhase =
            (Array.isArray(grid.beats) && Number.isFinite(Number(grid.beats[0]))
                ? Number(grid.beats[0])
                : Number(grid.offset) || 0) + (Number(grid.shift) || 0);

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
    let playback = use_context::<PlaybackUIState>();
    let theme = use_theme();

    use_effect(move || {
        // Поля PlaybackUIState — это Signal<T>, а не методы.
        let bpm_signal = playback.bpm;
        let grid_signal = playback.beat_grid;
        let grid_anchor_signal = playback.grid_anchor;
        let grid_tempo_signal = playback.grid_tempo;

        // Вызов сигнала читает значение и создаёт реактивную зависимость.
        let bpm = bpm_signal();
        let duration = total_duration();
        let snap = snap_enabled();
        let grid = grid_signal();
        let offset = grid.as_ref().map(|grid| grid.offset).unwrap_or(0.0);
        let manual_tempo = grid_tempo_signal();
        let tempo = resolved_grid_tempo(bpm, manual_tempo).unwrap_or(bpm);
        let shift = grid
            .as_ref()
            .map(|grid| grid_shift_for_anchor(grid, tempo, grid_anchor_signal()))
            .unwrap_or(0.0);

        let is_dark = theme.get() == Theme::Dark;

        spawn(async move {
            let eval = dioxus::document::eval(BEAT_GRID_JS);

            let _ = eval.send(serde_json::json!({
                "bpm": bpm,
                "tempo": tempo,
                "duration": duration,
                "snap": snap,
                "offset": offset,
                "shift": shift,
                "beats": grid.as_ref().map(|grid| &grid.beats),
                "bars": grid.as_ref().map(|grid| &grid.bars),
                "theme": if is_dark { "dark" } else { "light" },
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

#[cfg(test)]
mod grid_sync_tests {
    use super::{grid_shift_for_anchor, resolved_grid_tempo};
    use crate::shared::event_types::BeatGridData;

    fn grid() -> BeatGridData {
        BeatGridData {
            offset: 0.137,
            beats: vec![0.137, 5.137],
            bars: vec![0.137],
            downbeats: vec![0.137],
        }
    }

    #[test]
    fn shift_should_put_anchor_on_nearest_beat() {
        let shift = grid_shift_for_anchor(&grid(), 120.0, Some(1.3));

        assert!((shift - 0.163).abs() < 1e-4);
    }

    #[test]
    fn shift_should_be_zero_without_anchor() {
        assert_eq!(grid_shift_for_anchor(&grid(), 120.0, None), 0.0);
    }

    #[test]
    fn shift_should_keep_anchor_on_beat_after_tempo_change() {
        let anchor = 8.0;

        for tempo in [120.0, 127.4, 90.0] {
            let shift = grid_shift_for_anchor(&grid(), tempo, Some(anchor));
            let beat_duration = 60.0 / tempo;
            let phase = 0.137 + shift;
            let line = phase + ((anchor - phase) / beat_duration).round() * beat_duration;

            assert!((line - anchor).abs() < 1e-4);
        }
    }

    #[test]
    fn resolved_tempo_should_prefer_manual_value() {
        let tempo = resolved_grid_tempo(127.0, Some(120.0));

        assert_eq!(tempo, Some(120.0));
    }

    #[test]
    fn resolved_tempo_should_ignore_invalid_manual_value() {
        let tempo = resolved_grid_tempo(127.0, Some(0.0));

        assert_eq!(tempo, Some(127.0));
    }
}
