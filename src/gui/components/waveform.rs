use super::drag_and_drop::DragAndDropFile;
use super::loop_player::LoopPlayer;
use super::volume_meter::VolumeMeter;
use super::waveform_bins_canvas::WaveformBinsCanvas;
use dioxus::html::HasFileData;
use std::path::PathBuf;

use super::beat_grid::BeatGrid;
use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::{BeatGridData, PlayerEvent};

use crate::gui::layout::{
    BeatGridSnap, LoopRepeatRegion, PlaybackUIState, ShowBeatGrid, ShowTimeMarkers,
};
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[component]
pub fn Waveform() -> Element {
    let ctx = use_context::<PlayerContext>();
    let playback = use_context::<PlaybackUIState>();

    // Grid settings
    let show_grid = use_context::<ShowBeatGrid>().0;
    let grid_snap = use_context::<BeatGridSnap>().0;

    // Timeline settings & loop
    let show_time_markers = use_context::<ShowTimeMarkers>().0;
    let is_loop_enabled = use_context::<LoopRepeatRegion>().0;

    // Read reactive state from context
    let mut position_secs = playback.position_secs;
    let mut hover_position_secs = use_signal(|| None::<f32>);

    let mut duration_secs = playback.duration_secs;
    let mut has_track = playback.has_track;

    // Loop state (shared with LoopPlayer)

    // Drag state
    let dragging = use_signal(|| None::<&'static str>);
    // Overlay is visible only when there is no loaded track.
    let mut is_dragover_event = use_signal(move || !has_track());

    // Container rect for mouse calculations
    let mut container_rect: Signal<Option<(f32, f32)>> = use_signal(|| None);

    // Zoom state
    let zoom_level = use_signal(|| 1.0f32);
    let view_center = use_signal(|| 50.0f32);

    let waveform_peaks = playback.waveform_peaks;

    let pos = position_secs();
    let dur = duration_secs();

    let mut is_playing = use_signal(|| false);

    // Cursor progress in percent
    let cursor_progress = if dur > 0.0 { (pos / dur) * 100.0 } else { 0.0 };

    // Viewport calculations
    // let inv_zoom = zoom_level().recip();
    let half_view = 50.0 * zoom_level().recip();
    // let view_center_val = view_center();
    let view_start = (view_center() - half_view).clamp(0.0, 100.0 - 100.0 * zoom_level().recip());
    let view_end = (view_start + 100.0 * zoom_level().recip()).min(100.0);
    let view_range = view_end - view_start;

    // let total_bars = waveform_data().len();

    // 4. Background event listener for incoming updates from core
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started {
                        duration: d,
                        metadata: _,
                    } => {
                        duration_secs.set(d.as_secs_f32());
                        position_secs.set(0.0);
                        has_track.set(true);
                        is_playing.set(true);
                        is_dragover_event.set(false);
                    }
                    PlayerEvent::Paused => {
                        is_playing.set(false);
                    }

                    PlayerEvent::Resumed => {
                        is_playing.set(true);
                    }
                    PlayerEvent::Finished => {
                        is_playing.set(false);
                        position_secs.set(0.0); // сброс на 0;
                    }
                    _ => {} // ignore other variants
                }
            }
        }
    });

    use_future(move || {
        let pos = ctx.playback_progress.read().clone();
        async move {
            let current_position = pos.load(Ordering::Relaxed);

            // Важно для возврата на экран после паузы.
            position_secs.set(current_position as f32 / 1000.0);

            let mut last_position = current_position;

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

    // Time marks
    let time_marks = {
        if dur <= 0.0 {
            vec![(0.0, "00:00".to_string())]
        } else {
            let start_secs = dur * view_start / 100.0;
            let end_secs = dur * view_end / 100.0;
            let range_secs = end_secs - start_secs;
            if range_secs <= 0.0 {
                vec![(0.0, format_time(start_secs as u32))]
            } else {
                let max_marks = 15;
                let raw_step = range_secs / max_marks as f32;
                let nice_steps = [
                    0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0,
                    900.0, 1800.0, 3600.0,
                ];
                let step = nice_steps
                    .iter()
                    .find(|&&s| s as f32 >= raw_step)
                    .copied()
                    .unwrap_or(3600.0) as f32;

                // Убираем последнюю метку, если она ближе 20% к предыдущей
                let mut marks: Vec<(f32, String)> = (0..=max_marks)
                    .map(|i| {
                        let secs = (start_secs + i as f32 * step).min(end_secs);
                        let pct = ((secs - start_secs) / range_secs) * 100.0;
                        (pct, format_time(secs as u32))
                    })
                    .take_while(|(pct, _)| *pct < 99.5) // не включаем последнюю метку в конце
                    .collect();

                // Если меток слишком мало — добавим последнюю явно
                if marks.len() < 2 {
                    marks.push((100.0, format_time(end_secs as u32)));
                }

                marks
            }
        }
    };

    // Обработчик клавиатуры: ← → перемотка на 5с, Ctrl+←/→ на 30с
    let seek = move |e: KeyboardEvent| {
        // e.prevent_default();

        let key = e.key();
        let ctrl = e.modifiers().ctrl();
        let offset = if ctrl { 30.0 } else { 5.0 };

        match key {
            Key::ArrowLeft => {
                let new_pos = (pos - offset).max(0.0);
                let _ = ctx
                    .command_tx
                    .read()
                    .send(PlayerCommand::Seek(Duration::from_secs_f32(new_pos)));
            }
            Key::ArrowRight => {
                let new_pos = (pos + offset).min(dur - 0.1);
                let _ = ctx
                    .command_tx
                    .read()
                    .send(PlayerCommand::Seek(Duration::from_secs_f32(new_pos)));
            }
            _ => {}
        }
    };

    let hover_cursor_progress = hover_position_secs().map(|hover_position| {
        if dur <= 0.0 || view_range <= 0.0 {
            return 0.0;
        }

        let timeline_progress = (hover_position / dur) * 100.0;

        ((timeline_progress - view_start) / view_range * 100.0).clamp(0.0, 100.0)
    });

    rsx! {
        div { class: "w-full",
            tabindex: "0",
            onkeydown: seek,

            // Darg & Drop events in parent container
            ondragover: move |evt: DragEvent| {
                evt.prevent_default();
                is_dragover_event.set(true);
            },
            ondragleave: move |_| {
                if has_track() {
                    is_dragover_event.set(false);
                } else {
                    is_dragover_event.set(true);
                }
            },
            ondrop: move |evt: DragEvent| {
                evt.prevent_default();
                is_dragover_event.set(false);
                if let Some(file) = evt.files().first() {
                    let path = PathBuf::from(file.path());
                    let _ = ctx.command_tx.read().send(PlayerCommand::LoadFile(
                        path.to_string_lossy().to_string(),
                    ));
                }
            },

            if has_track() {
                // Loop Player component
                if is_loop_enabled() {
                    LoopPlayer { total_duration: dur, zoom: zoom_level, view_center, },
                }
            }

            div {
                class: "flex h-28 w-full min-w-0 items-stretch gap-2",
            div {
                class: "relative flex h-28 w-full min-w-0
                            items-center overflow-visible
                            rounded-2xl neumorph-sunken
                            hover:shadow",
                onmounted: move |el| {
                    spawn(async move {
                        if let Ok(rect) = el.get_client_rect().await {
                            container_rect.set(Some((rect.origin.x as f32, rect.size.width as f32)));
                        }
                    });
                },

                // Timeline click to seek
                onclick: {
                    move |e| {
                        if dragging().is_some() {
                            return;
                        }

                        let duration = duration_secs();
                        let Some((_, width)) = container_rect() else {
                            return;
                        };

                        if duration <= 0.0 || width <= 0.0 {
                            return;
                        }

                        let element_x = e.element_coordinates().x as f32;
                        let pct = (element_x / width).clamp(0.0, 1.0);

                        let zoom = zoom_level().max(1.0);
                        let visible_duration = duration / zoom;
                        let view_start_fraction = (view_start / 100.0).clamp(0.0, 1.0);

                        let raw_secs = (view_start_fraction * duration + pct * visible_duration)
                            .clamp(0.0, duration);

                        let secs = if show_grid() && grid_snap() {
                            let bpm_signal = playback.bpm;
                            let grid_signal = playback.beat_grid;

                            let bpm = bpm_signal();
                            let grid = grid_signal();

                            grid.as_ref().map_or(raw_secs, |grid| {
                                snap_to_grid(grid, bpm, raw_secs, width, zoom, duration)
                            })
                        } else {
                            raw_secs
                        };

                        let _ = ctx
                            .command_tx
                            .read()
                            .send(PlayerCommand::Seek(Duration::from_secs_f32(secs)));
                    }
                },

                // Timeline cursor hover
                onmousemove: move |event| {
                    let Some((_, width)) = container_rect() else {
                        return;
                    };

                    if width <= 0.0 || dur <= 0.0 {
                        return;
                    }

                    let x = event.element_coordinates().x as f32;
                    let viewport_progress = (x / width).clamp(0.0, 1.0);

                    // Convert mouse position inside the visible viewport
                    // into the position on the complete track timeline.
                    let timeline_progress =
                        (view_start + viewport_progress * view_range).clamp(0.0, 100.0);

                    let hover_position =
                        (timeline_progress / 100.0) * dur;

                    hover_position_secs.set(Some(hover_position));
                },

                onmouseleave: move |_| {
                    hover_position_secs.set(None);
                },


                if has_track() {

                    // Waveform bars
                    // WaveformBins { total_duration: duration_secs, cursor_progress }
                    WaveformBinsCanvas {
                        total_duration: duration_secs,
                        cursor_progress,
                        zoom: zoom_level,
                        view_center,
                        waveform_peaks,
                    }

                    // DONE
                    if show_grid() {
                        BeatGrid {
                            total_duration: duration_secs,
                            snap_enabled: grid_snap,
                        }
                    }

                    // Timeline cursor
                    div { class: "absolute top-0 bottom-0 flex items-center justify-center z-10 cursor-grab hover:scale-105 duration-20 ease-linear pointer-events-none",
                        // style: "left: {cursor_progress}%; transform: translateX(-50%);",
                        style: "left: var(--wave-cursor-pct, {cursor_progress}%); transform: translateX(-50%);",
                        title: "Timeline marker",
                        div { class: "w-[2px] h-full bg-primary shadow-[0_0_8px_rgba(84,98,83,0.5)]" }
                        div { class: "absolute -top-0 w-2 h-2 rounded-full bg-primary" }

                        // Метка времени над курсором
                        div { class: "absolute bottom-1 left-1/2 -translate-x-1/2 text-xs text-primary font-mono bg-primary/20 px-1 rounded-md text-center whitespace-nowrap z-10",
                            "{format_time(pos as u32)}"
                        }
                    }
                }

                // Hover preview cursor
                if let Some(hover_progress) = hover_cursor_progress {
                    if let Some(hover_position) = hover_position_secs() {
                        div {
                            class: "absolute top-0 bottom-0 flex items-center justify-center z-10 pointer-events-none opacity-45",
                            style: "left: {hover_progress}%; transform: translateX(-50%);",
                            title: "Preview position",

                            div {
                                class: "w-[2px] h-full bg-primary shadow-[0_0_8px_rgba(84,98,83,0.5)]"
                            }

                            div {
                                class: "absolute -top-0 w-2 h-2 rounded-full bg-primary"
                            }

                            div {
                                class: "absolute bottom-1 left-1/2 -translate-x-1/2 text-xs text-primary font-mono bg-primary/20 px-1 rounded-md text-center whitespace-nowrap z-10",
                                "{format_time(hover_position as u32)}"
                            }
                        }
                    }
                }

                if !has_track() || is_dragover_event() {
                    DragAndDropFile {
                        is_dragover_event: is_dragover_event,
                    }
                }

                if has_track() {
                    VolumeMeter {}
                }
            }
            }

            if has_track() {
                // Time scale
                if show_time_markers() {
                    div { class: "relative mt-1 h-6 w-full min-w-0
                            select-none overflow-visible",
                        {time_marks.into_iter().map(|(pct, label)| {
                            rsx! {
                                div {
                                    class: "absolute top-0 flex
                                        flex-col items-center",
                                    style: "left: {pct}%; transform: translateX(-50%);",
                                    div { class: "w-[1px] h-1.5 bg-secondary/40" }
                                    span {
                                        class: "mt-0.5 max-w-full truncate
                                            whitespace-nowrap text-[12px] text-secondary/60",
                                        "{label}"
                                    }
                                }
                            }
                        })}
                    }
                }
            }
        }
    }
}

fn format_time(secs: u32) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}

const GRID_MIN_SPACING_PX: f32 = 30.0;

fn grid_snap_interval(
    grid: &BeatGridData,
    bpm: f32,
    width: f32,
    zoom: f32,
    duration: f32,
) -> Option<(f32, f32)> {
    if bpm <= 0.0 || width <= 0.0 || duration <= 0.0 {
        return None;
    }

    let visible_duration = duration / zoom.max(1.0);
    let pixels_per_second = width / visible_duration.max(0.001);

    let beat_duration = 60.0 / bpm;
    let beat_phase = grid.beats.first().copied().unwrap_or(grid.offset);
    let bar_phase = grid.bars.first().copied().unwrap_or(beat_phase);

    let mut interval = beat_duration;
    let mut phase = beat_phase;

    // Та же логика, что в drawBeatGrid: на отдалённом масштабе
    // рисуются такты и кратные им линии.
    if interval * pixels_per_second < GRID_MIN_SPACING_PX {
        interval = beat_duration * 4.0;
        phase = bar_phase;
    }

    while interval * pixels_per_second < GRID_MIN_SPACING_PX {
        interval *= 2.0;
    }

    Some((interval, phase))
}

fn snap_to_grid(
    grid: &BeatGridData,
    bpm: f32,
    seconds: f32,
    width: f32,
    zoom: f32,
    duration: f32,
) -> f32 {
    let Some((interval, phase)) = grid_snap_interval(grid, bpm, width, zoom, duration) else {
        return seconds;
    };

    let index = ((seconds - phase) / interval).round();
    let snapped = phase + index * interval;

    snapped.clamp(0.0, duration)
}

#[cfg(test)]
mod grid_snap_tests {
    use super::snap_to_grid;
    use crate::shared::event_types::BeatGridData;

    fn grid() -> BeatGridData {
        BeatGridData {
            offset: 0.0,
            beats: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            bars: vec![0.0, 2.0],
            downbeats: vec![0.0],
        }
    }

    #[test]
    fn snap_should_use_beats_when_they_are_visible() {
        let snapped = snap_to_grid(&grid(), 120.0, 1.24, 1000.0, 10.0, 10.0);
        assert!((snapped - 1.0).abs() < 1e-4);
    }

    #[test]
    fn snap_should_follow_visible_grid_lines_when_zoomed_out() {
        // 120 s, 600 px, zoom 1 → 5 px/s. Шаг сетки доходит до 8 s.
        let snapped = snap_to_grid(&grid(), 120.0, 58.0, 600.0, 1.0, 120.0);
        assert!((snapped - 56.0).abs() < 1e-4);
    }
}
