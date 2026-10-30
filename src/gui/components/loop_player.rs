use crate::PlayerContext;
use crate::PlayerEvent;
use crate::gui::layout::{LoopDragging, LoopUIState};
use crate::shared::command_types::PlayerCommand;

use dioxus::core::use_drop;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::time::Duration;

pub(crate) const MIN_LOOP_LENGTH: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LoopBounds {
    pub start: f32,
    pub end: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DragMode {
    Move,
    Start,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DragSession {
    mode: DragMode,
    anchor_start: f32,
    anchor_end: f32,
    pointer_x: f32,
    container_width: f32,
    duration: f32,
    zoom: f32,
}

impl DragSession {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin(
        mode: DragMode,
        anchor_start: f32,
        anchor_end: f32,
        pointer_x: f32,
        container_width: f32,
        duration: f32,
        zoom: f32,
    ) -> Self {
        Self {
            mode,
            anchor_start,
            anchor_end,
            pointer_x,
            container_width: container_width.max(1.0),
            duration: duration.max(0.0),
            zoom: zoom.max(1.0),
        }
    }

    pub(crate) fn bounds_at(&self, client_x: f32) -> LoopBounds {
        let dx = client_x - self.pointer_x;
        let delta = (dx / self.container_width) * (self.duration / self.zoom);

        match self.mode {
            DragMode::Move => {
                move_loop_region(self.anchor_start, self.anchor_end, delta, self.duration)
            }
            DragMode::Start => LoopBounds {
                start: resize_loop_start(self.anchor_start, delta, self.anchor_end),
                end: self.anchor_end,
            },
            DragMode::End => LoopBounds {
                start: self.anchor_start,
                end: resize_loop_end(self.anchor_end, delta, self.anchor_start, self.duration),
            },
        }
    }

    pub(crate) fn cancelled_bounds(&self) -> LoopBounds {
        LoopBounds {
            start: self.anchor_start,
            end: self.anchor_end,
        }
    }
}

pub(crate) fn move_loop_region(
    anchor_start: f32,
    anchor_end: f32,
    delta: f32,
    duration: f32,
) -> LoopBounds {
    let width = (anchor_end - anchor_start).max(MIN_LOOP_LENGTH);
    let max_start = (duration - width).max(0.0);
    let start = (anchor_start + delta).clamp(0.0, max_start);

    LoopBounds {
        start,
        end: (start + width).min(duration),
    }
}

pub(crate) fn resize_loop_start(anchor_start: f32, delta: f32, end: f32) -> f32 {
    (anchor_start + delta).clamp(0.0, (end - MIN_LOOP_LENGTH).max(0.0))
}

pub(crate) fn resize_loop_end(anchor_end: f32, delta: f32, start: f32, duration: f32) -> f32 {
    (anchor_end + delta).clamp((start + MIN_LOOP_LENGTH).min(duration), duration)
}

pub(crate) fn time_to_viewport_pct(time: f32, duration: f32, zoom: f32, view_center: f32) -> f32 {
    if duration <= 0.0 {
        return 0.0;
    }

    let zoom = zoom.max(1.0);
    let visible = 100.0 / zoom;
    let view_start = (view_center - visible / 2.0).clamp(0.0, (100.0 - visible).max(0.0));
    let pct = ((time / duration) * 100.0 - view_start) / visible * 100.0;

    pct.clamp(0.0, 100.0)
}

#[allow(clippy::too_many_arguments)]
fn begin_drag(
    mode: DragMode,
    client_x: f32,
    pointer_id: i32,
    mut dragging: Signal<Option<DragMode>>,
    mut active_pointer_id: Signal<Option<i32>>,
    mut loop_dragging: Signal<bool>,
    mut drag_session: Signal<Option<DragSession>>,
    loop_start: Signal<f32>,
    loop_end: Signal<f32>,
    container_width: Signal<f32>,
    total_duration: f32,
    zoom: Signal<f32>,
) {
    if dragging().is_some() {
        return;
    }

    // Start the session synchronously from the cached width. Waiting for a
    // `getBoundingClientRect` round-trip here made the region lag behind the
    // cursor (especially while analysis was loading the CPU).
    let session = DragSession::begin(
        mode,
        loop_start(),
        loop_end(),
        client_x,
        container_width().max(1.0),
        total_duration,
        zoom(),
    );

    dragging.set(Some(mode));
    active_pointer_id.set(Some(pointer_id));
    loop_dragging.set(true);
    drag_session.set(Some(session));
}

#[allow(clippy::too_many_arguments)]
fn end_drag(
    commit: bool,
    mut dragging: Signal<Option<DragMode>>,
    mut drag_session: Signal<Option<DragSession>>,
    mut active_pointer_id: Signal<Option<i32>>,
    mut loop_dragging: Signal<bool>,
    mut loop_start: Signal<f32>,
    mut loop_end: Signal<f32>,
    ctx: PlayerContext,
) {
    if dragging().is_none() {
        return;
    }

    if !commit {
        if let Some(session) = drag_session() {
            let bounds = session.cancelled_bounds();
            loop_start.set(bounds.start);
            loop_end.set(bounds.end);
        }
    }

    dragging.set(None);
    drag_session.set(None);
    active_pointer_id.set(None);
    loop_dragging.set(false);

    if commit {
        let _ = ctx.command_tx.read().send(PlayerCommand::SetLoop(
            Duration::from_secs_f32(loop_start()),
            Duration::from_secs_f32(loop_end()),
        ));
    }
}

#[component]
pub fn LoopPlayer(total_duration: f32, zoom: Signal<f32>, mut view_center: Signal<f32>) -> Element {
    let ctx = use_context::<PlayerContext>();
    let loop_state = use_context::<LoopUIState>();

    let mut loop_start = loop_state.start;
    let mut loop_end = loop_state.end;
    let mut loop_dragging = use_context::<LoopDragging>().0;

    let dragging = use_signal(|| None::<DragMode>);
    let drag_session = use_signal(|| None::<DragSession>);
    let active_pointer_id = use_signal(|| None::<i32>);
    let mut is_loop_enabled = loop_state.enabled;

    // Export signals
    let mut export_started = use_signal(|| false);
    let mut export_progress = use_signal(|| 0.0f32);
    let mut exported_path = use_signal(|| None::<String>);

    let mut container_width = use_signal(|| 600.0f32);

    let zoom_factor = zoom().max(1.0);
    let view_center_value = view_center();
    let lp = time_to_viewport_pct(loop_start(), total_duration, zoom_factor, view_center_value);
    let rp = time_to_viewport_pct(loop_end(), total_duration, zoom_factor, view_center_value);

    // The labels are about 50 px wide with the current font and padding.
    const LOOP_LABEL_COLLISION_GAP_PX: f32 = 52.0;

    let loop_markers_distance_px = (rp - lp).abs() / 100.0 * container_width().max(1.0);
    let loop_labels_overlap = loop_markers_distance_px < LOOP_LABEL_COLLISION_GAP_PX;
    let loop_labels_center = (lp + rp) / 2.0;

    let send_set_loop = move || {
        let _ = ctx.command_tx.read().send(PlayerCommand::SetLoop(
            Duration::from_secs_f32(loop_start()),
            Duration::from_secs_f32(loop_end()),
        ));
    };

    use_effect(move || {
        let Some(_mode) = dragging() else {
            return;
        };
        let Some(pointer_id) = active_pointer_id() else {
            return;
        };

        spawn(async move {
            let mut eval = dioxus::document::eval(
                r#"
                let data = await dioxus.recv();
                const overlay = document.getElementById("loop-drag-overlay");

                if (overlay && data.pointerId != null) {
                    try {
                        overlay.setPointerCapture(data.pointerId);
                    } catch (_) {}
                }

                const cleanup = () => {
                    window.removeEventListener("pointerup", onUp, true);
                    window.removeEventListener("pointercancel", onCancel, true);
                    window.removeEventListener("keydown", onKey, true);

                    if (window.__waveLoopDragCleanup === cleanup) {
                        window.__waveLoopDragCleanup = null;
                    }
                };

                const onUp = () => {
                    cleanup();
                    dioxus.send("up");
                };

                const onCancel = () => {
                    cleanup();
                    dioxus.send("cancel");
                };

                const onKey = (event) => {
                    if (event.key === "Escape") {
                        cleanup();
                        dioxus.send("cancel");
                    }
                };

                window.__waveLoopDragCleanup = cleanup;
                window.addEventListener("pointerup", onUp, true);
                window.addEventListener("pointercancel", onCancel, true);
                window.addEventListener("keydown", onKey, true);
                "#,
            );

            let _ = eval.send(serde_json::json!({ "pointerId": pointer_id }));

            if let Ok(message) = eval.recv().await {
                let commit = matches!(
                    message,
                    serde_json::Value::String(ref value) if value == "up"
                );

                end_drag(
                    commit,
                    dragging,
                    drag_session,
                    active_pointer_id,
                    loop_dragging,
                    loop_start,
                    loop_end,
                    ctx,
                );
            }
        });
    });

    use_drop(move || {
        loop_dragging.set(false);
        let _ = dioxus::document::eval("window.__waveLoopDragCleanup?.();");
    });

    // 2. Делаем пропс реактивным с помощью use_reactive!
    // Как только total_duration изменится, этот эффект гарантированно перезапустится
    use_effect(use_reactive!(|(total_duration,)| {
        let duration = total_duration;

        // Ждем, пока трек загрузится (станет больше нуля)
        if duration > 0.0 {
            // Ситуация А: Самый первый запуск (петля еще на нулях)
            if loop_start() == 0.0 && loop_end() == 0.0 {
                let start_pos = duration * 0.75; // 1/4 от конца трека
                let end_pos = (start_pos + (duration * 0.85)).min(duration); // 10% длины петли

                loop_start.set(start_pos);
                loop_end.set(end_pos);
            }
            // Ситуация Б: Трек поменялся на более короткий, поджимаем маркеры вовнутрь
            else if loop_end() > duration {
                loop_end.set(duration);
                if loop_start() >= duration {
                    loop_start.set((duration - 1.0).max(0.0));
                }
            }
        }
    }));

    use_future(move || {
        // Ignore export events emitted before this component was mounted.
        let mut status_rx = ctx.status_rx.read().new_receiver();

        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::ExportStarted => {
                        export_started.set(true);
                        export_progress.set(0.05);
                        exported_path.set(None);
                    }

                    PlayerEvent::ExportProgress { progress } => {
                        export_progress.set(progress);
                    }

                    PlayerEvent::ExportFinished { path } => {
                        export_progress.set(1.0);
                        exported_path.set(Some(path.to_string_lossy().into_owned()));
                    }

                    PlayerEvent::ExportFailed => {
                        export_started.set(false);
                        export_progress.set(0.0);
                        exported_path.set(None);
                    }

                    _ => {}
                }
            }
        }
    });

    rsx! {
        div {
            id: "loop-drag-root",
            class: "absolute inset-0 z-10 select-none pointer-events-none",
            onmounted: move |el| {
                // Track the actual width continuously so a drag can start
                // synchronously and stays correct after window resizes.
                spawn(async move {
                    loop {
                        if let Ok(r) = el.get_client_rect().await {
                            let width = r.size.width.max(1.0) as f32;
                            let current = *container_width.peek();

                            if (current - width).abs() > 0.5 {
                                container_width.set(width);
                            }
                        }

                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                });
            },
            onclick: move |e| e.stop_propagation(),

            // same zoom at the inner loop region
            onwheel: move |e| {
                let delta = e.delta().strip_units();
                let delta_x = delta.x;
                let delta_y = delta.y;
                let shift_key = e.modifiers().shift();
                let client_x = e.data().client_coordinates().x as f32;

                let mut eval = dioxus::document::eval(
                    r#"
                    let data = await dioxus.recv();

                    const canvas =
                        document.getElementById("waveform-canvas");

                    if (!canvas || !window.zoomAt) {
                        return;
                    }

                    const rect =
                        canvas.getBoundingClientRect();

                    const width =
                        canvas.clientWidth || 1;

                    const localX =
                        data.clientX - rect.left;

                    const pct =
                        Math.max(0, Math.min(1, localX / width));

                        const result =
                            window.handleWaveWheel(
                                pct,
                                data.deltaX,
                                data.deltaY,
                                data.shiftKey
                            );

                        dioxus.send(result);
                    "#,
                );

                spawn(async move {
                    let _ = eval.send(serde_json::json!({
                        "clientX": client_x,
                        "deltaX": delta_x,
                        "deltaY": delta_y,
                        "shiftKey": shift_key,
                    }));

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
            },

            // Полноэкранный оверлей захвата: перехватывает движение мыши,
            // даже когда курсор слетает с маркеров/региона (как в крутилках)
            if dragging().is_some() {
                div {
                    id: "loop-drag-overlay",
                    class: if dragging() == Some(DragMode::Move) {
                        "fixed inset-0 z-50 cursor-grabbing bg-transparent select-none pointer-events-auto"
                    } else {
                        "fixed inset-0 z-50 cursor-w-resize bg-transparent select-none pointer-events-auto"
                    },
                    style: "-webkit-tap-highlight-color: transparent;",
                    onpointermove: move |e| {
                        if let Some(session) = drag_session() {
                            let bounds = session.bounds_at(e.data().client_coordinates().x as f32);
                            loop_start.set(bounds.start);
                            loop_end.set(bounds.end);
                        }
                    },
                    onpointerup: move |_| {
                        end_drag(
                            true,
                            dragging,
                            drag_session,
                            active_pointer_id,
                            loop_dragging,
                            loop_start,
                            loop_end,
                            ctx,
                        )
                    },
                    onpointercancel: move |_| {
                        end_drag(
                            false,
                            dragging,
                            drag_session,
                            active_pointer_id,
                            loop_dragging,
                            loop_start,
                            loop_end,
                            ctx,
                        )
                    },
                }
            },

            // Область петли
            div {
                class: "absolute inset-y-0 border-2 border-dashed rounded-xl border-primary/60 cursor-grab z-10 pointer-events-auto",
                class: if is_loop_enabled() { "bg-primary/20" } else { "bg-primary/5" },
                style: "left: {lp}%; right: calc(100% - {rp}%);",
                onpointerdown: move |e| {
                    e.stop_propagation();
                    begin_drag(
                        DragMode::Move,
                        e.data().client_coordinates().x as f32,
                        e.data().pointer_id(),
                        dragging,
                        active_pointer_id,
                        loop_dragging,
                        drag_session,
                        loop_start,
                        loop_end,
                        container_width,
                        total_duration,
                        zoom,
                    );
                },

                // Кнопка export cursor: not-allowed;
                div { //class: "absolute top-0 right-8 cursor-pointer material-symbols-outlined text-primary",
                    class: "absolute top-0 right-8 rounded-xl cursor-pointer hover:bg-secondary-container hover:scale-105 material-symbols-outlined text-primary",
                    style: "font-size: 20px;",
                    title: "Export audio loop",
                    // Не даём клику по кнопке запускать захват региона
                    onpointerdown: move |e| e.stop_propagation(),
                    onclick: move |_| {
                        if !export_started() {
                            send_set_loop();

                            let _ = ctx.command_tx.read().send(
                                PlayerCommand::ExportLoop(
                                    "exported_loop.wav".to_string(),
                                ),
                            );
                        }
                    },
                    span {
                        class: if export_started() {
                            "animate-pulse text-primary"
                        } else {
                            "text-primary"
                        },

                        if export_started() {
                            "progress_activity"
                        } else {
                            "save_as"
                        }
                    }
                }

                // Toggle loop icon
                div { class: if is_loop_enabled() {
                        "absolute top-0 right-1 hover:bg-secondary-container rounded-xl cursor-pointer hover:scale-105 material-symbols-outlined bg-primary"
                    } else {
                        "absolute top-0 right-1 hover:bg-secondary-container rounded-xl cursor-pointer hover:scale-105 material-symbols-outlined text-primary"
                    },
                    style: "font-size: 20px;",
                    title: "Toggle loop",
                    // Не даём клику по кнопке запускать захват региона
                    onpointerdown: move |e| e.stop_propagation(),
                    onclick: {
                        move |_| {
                            let _ = ctx.command_tx.read().send(PlayerCommand::ToggleLoop(!is_loop_enabled()));
                            is_loop_enabled.set(!is_loop_enabled());
                        }
                    },
                    span {
                        class: if is_loop_enabled() {
                            "text-surface-bright hover:text-primary"
                        } else {
                            "text-primary"
                        },
                        "laps"
                    }
                }
            }

            // Progress pop-up
            if export_started() {
                // Backdrop
                div {
                    class: "pointer-events-auto cursor-default fixed inset-0 z-50 bg-black/30 backdrop-blur-sm",
                    style: "z-index: 100;",
                }

                 // Progress modal
                div {
                    class: "pointer-events-auto cursor-default fixed p-5 top-1/2 left-[60%] z-60 w-[420px] max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-outline-variant bg-surface-container-high shadow-2xl",
                    style: "z-index: 110;",

                    onpointerdown: move |evt| {
                        evt.stop_propagation();
                    },

                    div {
                        class: "flex items-center justify-between gap-3",

                        div {
                            class: "flex items-center gap-2",

                            span {
                                class: "material-symbols-outlined text-primary",
                                if exported_path().is_some() {
                                    "check_circle"
                                } else {
                                    "progress_activity"
                                }
                            }

                            div {
                                class: "text-sm font-medium",

                                if exported_path().is_some() {
                                    "Export complete"
                                } else {
                                    "Exporting loop..."
                                }
                            }
                        }

                        button {
                            class: "flex h-7 w-7 items-center justify-center rounded-full text-on-surface-variant hover:bg-surface-variant hover:text-on-surface cursor-pointer",
                            title: "Close",

                            onclick: move |evt| {
                                evt.stop_propagation();

                                export_started.set(false);
                                export_progress.set(0.0);
                                exported_path.set(None);
                            },

                            span {
                                class: "material-symbols-outlined text-[18px]",
                                "close"
                            }
                        }
                    }

                    div {
                        class: "mt-3 h-2 overflow-hidden rounded-full bg-surface-variant",

                        div {
                            class: "h-full rounded-full bg-primary transition-all duration-300",
                            style: "width: {export_progress() * 100.0}%;",
                        }
                    }

                    div {
                        class: "mt-2 w-full max-w-full overflow-hidden break-all whitespace-normal text-xs leading-5 text-on-surface-variant",

                        if let Some(path) = exported_path() {
                            "{path}"
                        } else {
                            "{(export_progress() * 100.0) as u32}%"
                        }
                    }

                    if let Some(path) = exported_path() {
                        div {
                            class: "mt-3 flex justify-end",

                            button {
                                class: "flex items-center shrink-0 gap-2 rounded-xl bg-primary px-3 py-2 text-sm text-on-primary hover:brightness-110 cursor-pointer",

                                onclick: move |evt| {
                                    evt.stop_propagation();

                                    open_file_location(path.clone());
                                },

                                span {
                                    class: "material-symbols-outlined text-[18px]",
                                    "folder_open"
                                }

                                "Open folder"
                            }
                        }
                    }
                }
            }

            // When the markers are close, show both labels as one vertical stack.
            if loop_labels_overlap {
                div {
                    class: "pointer-events-none absolute z-30 flex
                            -translate-x-1/2 flex-col items-center gap-0.5",

                    style: "left: {loop_labels_center}%; bottom: calc(100% + 4px);",

                    div {
                        class: "rounded-md bg-primary/10 px-1 text-center
                                font-mono text-xs text-primary whitespace-nowrap",

                        "{format_time(loop_start() as u32)}"
                    }

                    div {
                        class: "rounded-md bg-primary/10 px-1 text-center
                                font-mono text-xs text-primary whitespace-nowrap",

                        "{format_time(loop_end() as u32)}"
                    }
                }
            }

            // Левый маркер
            div {
                class: "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 cursor-w-resize z-20 pointer-events-auto",
                style: "left: {lp}%;",
                onpointerdown: move |e| {
                    e.stop_propagation();
                    begin_drag(
                        DragMode::Start,
                        e.data().client_coordinates().x as f32,
                        e.data().pointer_id(),
                        dragging,
                        active_pointer_id,
                        loop_dragging,
                        drag_session,
                        loop_start,
                        loop_end,
                        container_width,
                        total_duration,
                        zoom,
                    );
                },
                // Метка над левым маркером
                if !loop_labels_overlap {
                    div {
                        class: "absolute -top-15 left-1/2 -translate-x-1/2
                                text-xs text-primary font-mono bg-primary/10
                                px-1 rounded-md text-center whitespace-nowrap",

                        "{format_time(loop_start() as u32)}"
                    }
                }

                span { class: "material-symbols-outlined text-primary/90 text-[48px] hover:scale-115", "drag_indicator" }
            }

            // Правый маркер
            div {
                class: "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 cursor-w-resize z-20 pointer-events-auto",
                style: "left: {rp}%;",
                onpointerdown: move |e| {
                    e.stop_propagation();
                    begin_drag(
                        DragMode::End,
                        e.data().client_coordinates().x as f32,
                        e.data().pointer_id(),
                        dragging,
                        active_pointer_id,
                        loop_dragging,
                        drag_session,
                        loop_start,
                        loop_end,
                        container_width,
                        total_duration,
                        zoom,
                    );
                },
                // Метка над правым маркером
                if !loop_labels_overlap {
                    div {
                        class: "absolute -top-15 left-1/2 -translate-x-1/2
                                text-xs text-primary font-mono bg-primary/10
                                px-1 rounded-md text-center whitespace-nowrap",

                        "{format_time(loop_end() as u32)}"
                    }
                }

                span { class: "material-symbols-outlined text-primary/90 text-[48px] hover:scale-115", "drag_indicator" }
            }
        }
    }
}

fn format_time(secs: u32) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}

#[cfg(target_os = "windows")]
fn open_file_location(path: String) {
    let path = std::path::PathBuf::from(path);

    let Some(path) = path.to_str() else {
        return;
    };

    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn();
}

#[cfg(target_os = "macos")]
fn open_file_location(path: String) {
    let _ = std::process::Command::new("open")
        .args(["-R", &path])
        .spawn();
}

#[cfg(target_os = "linux")]
fn open_file_location(path: String) {
    let path = std::path::PathBuf::from(path);

    if let Some(parent) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
}

#[cfg(target_os = "android")]
fn open_file_location(_path: String) {}

#[cfg(test)]
mod loop_tests {
    use super::{
        DragMode, DragSession, LoopBounds, move_loop_region, resize_loop_end, resize_loop_start,
        time_to_viewport_pct,
    };

    #[test]
    fn viewport_pct_should_show_the_whole_track_at_zoom_one() {
        assert_eq!(time_to_viewport_pct(0.0, 100.0, 1.0, 50.0), 0.0);
        assert_eq!(time_to_viewport_pct(50.0, 100.0, 1.0, 50.0), 50.0);
        assert_eq!(time_to_viewport_pct(100.0, 100.0, 1.0, 50.0), 100.0);
    }

    #[test]
    fn viewport_pct_should_follow_zoom_and_center() {
        assert_eq!(time_to_viewport_pct(25.0, 100.0, 2.0, 50.0), 0.0);
        assert_eq!(time_to_viewport_pct(50.0, 100.0, 2.0, 50.0), 50.0);
        assert_eq!(time_to_viewport_pct(75.0, 100.0, 2.0, 50.0), 100.0);
        assert_eq!(time_to_viewport_pct(50.0, 100.0, 2.0, 100.0), 0.0);
        assert_eq!(time_to_viewport_pct(100.0, 100.0, 2.0, 100.0), 100.0);
    }

    #[test]
    fn viewport_pct_should_be_zero_without_a_track() {
        assert_eq!(time_to_viewport_pct(10.0, 0.0, 1.0, 50.0), 0.0);
    }

    #[test]
    fn moving_the_region_should_preserve_length_and_clamp() {
        assert_eq!(
            move_loop_region(10.0, 20.0, 5.0, 100.0),
            LoopBounds {
                start: 15.0,
                end: 25.0
            }
        );
        assert_eq!(
            move_loop_region(0.0, 10.0, -5.0, 100.0),
            LoopBounds {
                start: 0.0,
                end: 10.0
            }
        );
        assert_eq!(
            move_loop_region(90.0, 100.0, 10.0, 100.0),
            LoopBounds {
                start: 90.0,
                end: 100.0
            }
        );
    }

    #[test]
    fn resizing_markers_should_enforce_minimum_length_and_bounds() {
        assert_eq!(resize_loop_start(10.0, 15.0, 20.0), 19.5);
        assert_eq!(resize_loop_start(10.0, -20.0, 20.0), 0.0);
        assert_eq!(resize_loop_end(20.0, -20.0, 10.0, 100.0), 10.5);
        assert_eq!(resize_loop_end(90.0, 20.0, 10.0, 100.0), 100.0);
    }

    #[test]
    fn drag_session_should_compute_move_relative_to_pointer_start() {
        let session = DragSession::begin(DragMode::Move, 10.0, 20.0, 50.0, 100.0, 100.0, 1.0);

        assert_eq!(
            session.bounds_at(60.0),
            LoopBounds {
                start: 20.0,
                end: 30.0
            }
        );
        assert_eq!(
            session.bounds_at(50.0),
            LoopBounds {
                start: 10.0,
                end: 20.0
            }
        );
    }

    #[test]
    fn drag_session_should_use_visible_duration_at_zoom() {
        let session = DragSession::begin(DragMode::Move, 10.0, 20.0, 50.0, 100.0, 100.0, 2.0);

        assert_eq!(
            session.bounds_at(100.0),
            LoopBounds {
                start: 35.0,
                end: 45.0
            }
        );
    }

    #[test]
    fn drag_session_cancel_should_restore_the_anchor() {
        let session = DragSession::begin(DragMode::Start, 10.0, 20.0, 50.0, 100.0, 100.0, 1.0);

        assert_eq!(
            session.bounds_at(90.0),
            LoopBounds {
                start: 19.5,
                end: 20.0
            }
        );
        assert_eq!(
            session.cancelled_bounds(),
            LoopBounds {
                start: 10.0,
                end: 20.0
            }
        );
    }
}
