use crate::PlayerContext;
use crate::PlayerEvent;
use crate::gui::layout::LoopUIState;
use crate::shared::command_types::PlayerCommand;

use dioxus::prelude::*;
use futures_util::StreamExt;
use std::time::Duration;

#[component]
pub fn LoopPlayer(total_duration: f32) -> Element {
    let ctx = use_context::<PlayerContext>();
    let loop_state = use_context::<LoopUIState>();

    let mut loop_start = loop_state.start;
    let mut loop_end = loop_state.end;

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

    let mut dragging = use_signal(|| None::<&'static str>);
    let mut anchor_x = use_signal(|| 0.0f32);
    let mut anchor_val = use_signal(|| 0.0f32); // loop_start для L, loop_end для R
    let mut anchor_l = use_signal(|| 0.0f32);
    let mut anchor_r = use_signal(|| 0.0f32);
    let mut is_loop_enabled = loop_state.enabled;

    // Export signals
    let mut export_started = use_signal(|| false);
    let mut export_progress = use_signal(|| 0.0f32);
    let mut exported_path = use_signal(|| None::<String>);

    // Контейнер 600px, родитель по центру — cl примерно 300px при 1200px экране
    // Но чтобы не гадать — просто читаем из onmounted один раз
    let mut cl = use_signal(|| 0.0f32);
    let mut container_width = use_signal(|| 600.0f32);

    let lp = if total_duration > 0.0 {
        loop_start() / total_duration * 100.0
    } else {
        0.0
    };
    let rp = if total_duration > 0.0 {
        loop_end() / total_duration * 100.0
    } else {
        100.0
    };

    // The labels are about 50 px wide with the current font and padding.
    const LOOP_LABEL_COLLISION_GAP_PX: f32 = 52.0;

    let loop_markers_distance_px = ((rp - lp).max(0.0) / 100.0) * container_width().max(1.0);

    let loop_labels_overlap = loop_markers_distance_px < LOOP_LABEL_COLLISION_GAP_PX;

    let loop_labels_center = (lp + rp) / 2.0;

    let send_set_loop = move || {
        let _ = ctx.command_tx.read().send(PlayerCommand::SetLoop(
            Duration::from_secs_f32(loop_start()),
            Duration::from_secs_f32(loop_end()),
        ));
    };

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
            class: "absolute w-full max-w-[600px] h-28 rounded-xl select-none pointer-events-auto",
            "data-loop-start": "{loop_start()}",
            "data-loop-end": "{loop_end()}",
            // style: "width: 600px;",
            onmounted: move |el| {
                spawn(async move {
                    if let Ok(r) = el.get_client_rect().await {
                        cl.set(r.origin.x as f32);
                        container_width.set(r.size.width as f32);
                    }
                });
            },

            /* onmousemove: move |e| {
                if let Some(mode) = dragging() {
                    let left = cl();
                    let cx = e.data().client_coordinates().x as f32 - left;
                    // let pct = (cx / 600.0).clamp(0.0, 1.0);
                    // let secs = pct * total_duration;

                    match mode {
                        "M" => {
                            let dx = cx - anchor_x();
                            let ds = (dx / 600.0) * total_duration;
                            let rw = anchor_r() - anchor_l();
                            let mut ns = anchor_l() + ds;
                            ns = ns.max(0.0).min((total_duration - rw).max(0.0));
                            loop_start.set(ns);
                            loop_end.set(ns + rw);
                        }
                        "L" => {
                            let dx = cx - anchor_x();
                            let ds = (dx / 600.0) * total_duration;
                            let new_l = (anchor_val() + ds)
                                .max(0.0)
                                .min((loop_end() - 0.5).max(0.0));
                            loop_start.set(new_l);
                        }
                        "R" => {
                            let dx = cx - anchor_x();
                            let ds = (dx / 600.0) * total_duration;
                            let new_r = (anchor_val() + ds)
                                .max(loop_start() + 0.5)
                                .min(total_duration);
                            loop_end.set(new_r);
                        }
                        _ => {}
                    }
                }
            },

            onmouseup: move |_| {
                if dragging().is_some() {
                    dragging.set(None);
                    send_set_loop();
                }
            },
            onmouseleave: move |_| {
                if dragging().is_some() {
                    dragging.set(None);
                }
            }, */

            // Полноэкранный оверлей захвата: перехватывает движение мыши,
            // даже когда курсор слетает с маркеров/региона (как в крутилках)
            if dragging().is_some() {
                div {
                    // class: "fixed inset-0 z-50 cursor-grab bg-transparent select-none",
                    class: if dragging() == Some("M") {
                        "fixed inset-0 z-50 cursor-grabbing bg-transparent select-none"
                    } else {
                        "fixed inset-0 z-50 cursor-w-resize bg-transparent select-none"
                    },
                    style: "-webkit-tap-highlight-color: transparent;",
                    onmousemove: move |e| {
                        if let Some(mode) = dragging() {
                            let left = cl();
                            let cx = e.data().client_coordinates().x as f32 - left;
                            // let container_width = 600.0;
                            let container_width = container_width().max(1.0);

                            match mode {
                                "M" => {
                                    let dx = cx - anchor_x();
                                    let ds = (dx / container_width) * total_duration;
                                    let rw = anchor_r() - anchor_l();
                                    let mut ns = anchor_l() + ds;
                                    ns = ns.max(0.0).min((total_duration - rw).max(0.0));
                                    loop_start.set(ns);
                                    loop_end.set(ns + rw);
                                }
                                "L" => {
                                    let dx = cx - anchor_x();
                                    let ds = (dx / container_width) * total_duration;
                                    let new_l = (anchor_val() + ds)
                                        .max(0.0)
                                        .min((loop_end() - 0.5).max(0.0));
                                    loop_start.set(new_l);
                                }
                                "R" => {
                                    let dx = cx - anchor_x();
                                    let ds = (dx / container_width) * total_duration;
                                    let new_r = (anchor_val() + ds)
                                        .max(loop_start() + 0.5)
                                        .min(total_duration);
                                    loop_end.set(new_r);
                                }
                                _ => {}
                            }
                        }
                    },
                    onmouseup: move |_| {
                        if dragging().is_some() {
                            dragging.set(None);
                            send_set_loop();
                        }
                    },
                    onmouseleave: move |_| {
                        if dragging().is_some() {
                            dragging.set(None);
                        }
                    },
                }
            },

            // Область петли
            div {
                class: "absolute inset-y-0 border-2 border-dashed rounded-xl border-primary/60 cursor-grab z-10",
                class: if is_loop_enabled() { "bg-primary/20" } else { "bg-primary/5" },
                style: "left: {lp}%; right: {100.0 - rp}%;",
                // style: "left: var(--wave-loop-start, {lp}%); right: calc(100% - var(--wave-loop-end, {rp}%));",
                onmousedown: move |e| {
                    e.stop_propagation();
                    dragging.set(Some("M"));
                    let left = cl();
                    anchor_x.set(e.data().client_coordinates().x as f32 - left);
                    anchor_l.set(loop_start());
                    anchor_r.set(loop_end());
                },

                // Кнопка export cursor: not-allowed;
                div { //class: "absolute top-0 right-8 cursor-pointer material-symbols-outlined text-primary",
                    class: "absolute top-0 right-8 rounded-xl cursor-pointer hover:bg-secondary-container hover:scale-105 material-symbols-outlined text-primary",
                    style: "font-size: 20px;",
                    title: "Export audio loop",
                    // Не даём клику по кнопке запускать захват региона
                    onmousedown: move |e| e.stop_propagation(),
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
                    onmousedown: move |e| e.stop_propagation(),
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

                    onmousedown: move |evt| {
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
                class: "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 cursor-w-resize z-20",
                style: "left: {lp}%;",
                // style: "left: var(--wave-loop-start, {lp}%);",
                onmousedown: move |e| {
                    e.stop_propagation();
                    dragging.set(Some("L"));
                    let left = cl();
                    anchor_x.set(e.data().client_coordinates().x as f32 - left);
                    anchor_val.set(loop_start());
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
                class: "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 cursor-w-resize z-20",
                style: "left: {rp}%;",
                // style: "left: var(--wave-loop-end, {rp}%);",
                onmousedown: move |e| {
                    e.stop_propagation();
                    dragging.set(Some("R"));
                    let left = cl();
                    anchor_x.set(e.data().client_coordinates().x as f32 - left);
                    anchor_val.set(loop_end());
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
