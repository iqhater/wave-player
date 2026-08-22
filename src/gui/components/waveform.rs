use super::drag_and_drop::DragAndDropFile;
use super::loop_player::LoopPlayer;
use super::waveform_bins_canvas::WaveformBinsCanvas;
use dioxus::html::HasFileData;
use std::path::PathBuf;

use super::beat_grid::BeatGrid;
use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::PlayerEvent;

use dioxus::prelude::*;
use futures_util::StreamExt;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[component]
pub fn Waveform() -> Element {
    let ctx = use_context::<PlayerContext>();
    // let cmd_tx = ctx.command_tx;
    // let init_playback = ctx.playback_progress.read().clone();

    // Read reactive state from context
    let mut position_secs = use_signal(|| 0.0);
    /* let mut position_secs = use_signal(move || {
        Duration::from_millis(init_playback.load(Ordering::Relaxed) as u64).as_secs_f32()
    }); */
    let mut duration_secs = use_signal(|| 0.0);
    let mut has_track = use_signal(|| false);

    // let waveform_data = ctx.waveform_data;

    // Loop state (shared with LoopPlayer)

    // Drag state
    let mut dragging = use_signal(|| None::<&'static str>);
    let mut last_mouse_x = use_signal(|| 0.0 as f32);
    let mut is_dragover_event = use_signal(|| true);

    // Container rect for mouse calculations
    let mut container_rect: Signal<Option<(f32, f32)>> = use_signal(|| None);

    // Zoom state
    let mut zoom_level = use_signal(|| 1.0f32);
    let mut view_center = use_signal(|| 50.0f32);

    let pos = position_secs();
    let dur = duration_secs();

    let mut is_playing = use_signal(|| false);

    // Cursor progress in percent
    let cursor_progress = if dur > 0.0 { (pos / dur) * 100.0 } else { 0.0 };

    // Viewport calculations
    let inv_zoom = zoom_level().recip();
    let half_view = 50.0 * inv_zoom;
    let view_center_val = view_center();
    let view_start = (view_center_val - half_view).clamp(0.0, 100.0 - 100.0 * inv_zoom);
    let view_end = (view_start + 100.0 * inv_zoom).min(100.0);
    let view_range = view_end - view_start;

    // let total_bars = waveform_data().len();

    // 4. Background event listener for incoming updates from core
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    /* PlayerEvent::LoopSet { start, end } => {
                        loop_start.set(start.as_secs_f32());
                        loop_end.set(end.as_secs_f32());
                    }
                    PlayerEvent::LoopEnabled {
                        is_loop: loop_enabled,
                    } => {
                        is_loop_enabled.set(loop_enabled);
                    } */
                    PlayerEvent::Started {
                        duration: d,
                        metadata: _,
                    } => {
                        duration_secs.set(d.as_secs_f32());
                        position_secs.set(0.0);
                        has_track.set(true);
                        is_playing.set(true);
                        is_dragover_event.set(false);

                        // is_playing.set(true);
                        println!("DEBUG EVENT: PlayerEvent::Started");
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
                        println!("DEBUG EVENT: PlayerEvent::Finished");
                    }
                    _ => {} // ignore other variants
                }
            }
        }
    });

    use_future(move || {
        let pos = ctx.playback_progress.read().clone();
        async move {
            let mut last_position = pos.load(Ordering::Relaxed);

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
                tokio::time::sleep(Duration::from_millis(16)).await;
                position_secs.set(pos.load(Ordering::Relaxed) as f32 / 1000.0);
            }
        } */
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
        // let dur = duration().as_secs_f32();

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
                LoopPlayer { total_duration: dur },
            }

            div {
                class: "w-full h-28 relative flex items-center neumorph-sunken rounded-2xl hover:shadow",
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
                            // dragging.set(None);
                            return;
                        }

                        let element_x = e.element_coordinates().x as f32;

                        // Ширину берём прямо из canvas (в тех же CSS-пикселях, что и offsetX)
                        /* let mut size_eval = dioxus::document::eval(
                            r#"
                            let c = document.getElementById("waveform-canvas");
                            dioxus.send(c ? c.clientWidth : 0);
                            "#
                        ); */
                        let mut snap_eval = dioxus::document::eval(
                            r#"
                            let d = await dioxus.recv();

                            const canvas =
                                document.getElementById("waveform-canvas");

                            const width =
                                canvas ? canvas.clientWidth : 1;

                            const pct =
                                Math.max(0, Math.min(1, d.x / width));

                            const rawSecs =
                                window.waveTimeAtViewportPct
                                    ? window.waveTimeAtViewportPct(pct)
                                    : d.duration * pct;

                            const snappedSecs =
                                window.snapToGrid
                                    ? window.snapToGrid(rawSecs)
                                    : rawSecs;

                            dioxus.send(snappedSecs);
                            "#
                        );

                        spawn(async move {
                            let _ = snap_eval.send(serde_json::json!({
                                "x": element_x,
                                "duration": duration_secs(),
                            }));
                            if let Ok(serde_json::Value::Number(s)) = snap_eval.recv().await {
                                if let Some(secs) = s.as_f64() {
                                    let _ = ctx.command_tx.read().send(PlayerCommand::Seek(
                                        Duration::from_secs_f32(secs as f32),
                                    ));
                                }
                            }
                            /* if let Ok(serde_json::Value::Number(w)) = size_eval.recv().await {
                                if let Some(w) = w.as_f64() {
                                    if w > 0.0 {
                                        let pct = (element_x / w as f32).clamp(0.0, 1.0);
                                        const rawSecs = window.waveTimeAtViewportPct
                                            ? window.waveTimeAtViewportPct(pct)
                                            : d.duration * pct;
                                        // Магнит к сетке (уважает snap-настройку внутри JS)
                                        let mut snap_eval = dioxus::document::eval(
                                            r#"
                                            let data = await dioxus.recv();
                                            dioxus.send(window.snapToGrid ? window.snapToGrid(data.secs) : data.secs);
                                            "#
                                        );
                                        let _ = snap_eval.send(serde_json::json!({ "secs": raw_secs }));
                                        if let Ok(serde_json::Value::Number(s)) = snap_eval.recv().await {
                                            if let Some(secs) = s.as_f64() {
                                                let _ = ctx.command_tx.read().send(PlayerCommand::Seek(
                                                    Duration::from_secs_f32(secs as f32),
                                                ));
                                            }
                                        }
                                        /* let pct = (element_x / w as f32).clamp(0.0, 1.0);
                                        let _ = ctx.command_tx.read().send(PlayerCommand::Seek(
                                            Duration::from_secs_f32(duration_secs() * pct),
                                        )); */
                                    }
                                }
                            } */
                        });
                    }
                },

                if has_track() {

                    // Waveform bars
                    // WaveformBins { total_duration: duration_secs, cursor_progress }
                    WaveformBinsCanvas { total_duration: duration_secs, cursor_progress }

                    // TODO: fix gird zoom and calcualtions
                    BeatGrid { total_duration: duration_secs, snap_enabled: true }

                    // Timeline cursor
                    div { class: "absolute top-0 bottom-0 flex items-center justify-center z-25 cursor-grab hover:scale-105 duration-20 ease-linear pointer-events-none",
                        // style: "left: {cursor_progress}%; transform: translateX(-50%);",
                        style: "left: var(--wave-cursor-pct, {cursor_progress}%); transform: translateX(-50%);",
                        title: "Timeline marker",
                        div { class: "w-[2px] h-full bg-primary shadow-[0_0_8px_rgba(84,98,83,0.5)]" }
                        div { class: "absolute -top-0 w-2 h-2 rounded-full bg-primary" }

                        // Метка времени над курсором
                        div { class: "absolute bottom-1 left-1/2 -translate-x-1/2 text-xs text-primary font-mono bg-primary/20 px-1 rounded-md text-center whitespace-nowrap z-50",
                            "{format_time(pos as u32)}"
                        }
                    }
                }
                DragAndDropFile { is_dragover_event: is_dragover_event }
            }

            if has_track() {
                // Time scale
                div { class: "relative w-full h-6 mt-1 select-none",
                    {time_marks.into_iter().map(|(pct, label)| {
                        rsx! {
                            div {
                                class: "absolute top-0 flex flex-col items-center",
                                style: "left: {pct}%; transform: translateX(-50%);",
                                div { class: "w-[1px] h-1.5 bg-secondary/40" }
                                span {
                                    class: "text-[12px] text-secondary/60 mt-0.5 whitespace-nowrap",
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

fn format_time(secs: u32) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}
