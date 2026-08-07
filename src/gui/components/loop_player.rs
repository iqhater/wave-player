use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;
use dioxus::prelude::*;
// use futures_util::StreamExt;
use std::time::Duration;

#[component]
pub fn LoopPlayer(total_duration: f32) -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut loop_start = use_signal(|| total_duration * 0.75);
    let mut loop_end = use_signal(|| (total_duration * 0.75) * 0.1);

    // 2. Делаем пропс реактивным с помощью use_reactive!
    // Как только total_duration изменится, этот эффект гарантированно перезапустится
    use_effect(use_reactive!(|(total_duration,)| {
        let duration = total_duration;

        // Ждем, пока трек загрузится (станет больше нуля)
        if duration > 0.0 {
            // Ситуация А: Самый первый запуск (петля еще на нулях)
            if loop_start() == 0.0 && loop_end() == 0.0 {
                let start_pos = duration * 0.75; // 1/4 от конца трека
                let end_pos = (start_pos + (duration * 0.10)).min(duration); // 10% длины петли

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
    let mut is_loop_enabled = use_signal(|| false);

    // Контейнер 600px, родитель по центру — cl примерно 300px при 1200px экране
    // Но чтобы не гадать — просто читаем из onmounted один раз
    let mut cl = use_signal(|| 0.0f32);

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

    let send_set_loop = move || {
        let _ = ctx.command_tx.read().send(PlayerCommand::SetLoop(
            Duration::from_secs_f32(loop_start()),
            Duration::from_secs_f32(loop_end()),
        ));
    };

    // Слушаем статусы
    /* use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started {
                        duration: d,
                        metadata: _,
                    } => {
                        duration.set(d);
                    }
                    _ => {} // ignore other variants
                }
            }
        }
    }); */

    rsx! {
        div {
            class: "absolute w-[600px] h-28 rounded-xl select-none pointer-events-auto",
            // style: "width: 600px;",
            onmounted: move |el| {
                spawn(async move {
                    if let Ok(r) = el.get_client_rect().await {
                        cl.set(r.origin.x as f32);
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
                        "fixed inset-0 z-50 cursor-grab bg-transparent select-none"
                    } else {
                        "fixed inset-0 z-50 cursor-w-resize bg-transparent select-none"
                    },
                    style: "-webkit-tap-highlight-color: transparent;",
                    onmousemove: move |e| {
                        if let Some(mode) = dragging() {
                            let left = cl();
                            let cx = e.data().client_coordinates().x as f32 - left;

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
                    },
                }
            },

            // Область петли
            div {
                class: "absolute inset-y-0 border-2 border-dashed rounded-xl border-primary/60 cursor-grab z-10",
                class: if is_loop_enabled() { "bg-primary/20" } else { "bg-primary/5" },
                style: "left: {lp}%; right: {100.0 - rp}%;",
                onmousedown: move |e| {
                    e.stop_propagation();
                    dragging.set(Some("M"));
                    let left = cl();
                    anchor_x.set(e.data().client_coordinates().x as f32 - left);
                    anchor_l.set(loop_start());
                    anchor_r.set(loop_end());
                },

                // Кнопка export
                div { class: "absolute top-0 right-8 cursor-pointer material-symbols-outlined text-primary",
                    style: "font-size: 20px; cursor: not-allowed;",
                    title: "Export audio loop",
                    // Не даём клику по кнопке запускать захват региона
                    onmousedown: move |e| e.stop_propagation(),
                    onclick: move |_| {},
                    span { "save_as" }
                }

                // Кнопка toggle loop
                /* div { class: "absolute top-0 right-1 cursor-pointer material-symbols-outlined text-primary text-[22px]",
                    onclick: move |_| {
                        let new_val = !is_loop_enabled();
                        is_loop_enabled.set(new_val);
                        let _ = ctx.command_tx.read().send(PlayerCommand::ToggleLoop(new_val));
                    },
                    span { if is_loop_enabled() { "repeat_on" } else { "laps" } }
                } */
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

            // Левый маркер
            div {
                class: "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 cursor-w-resize z-20",
                style: "left: {lp}%;",
                onmousedown: move |e| {
                    e.stop_propagation();
                    dragging.set(Some("L"));
                    let left = cl();
                    anchor_x.set(e.data().client_coordinates().x as f32 - left);
                    anchor_val.set(loop_start());
                },
                // Метка над левым маркером
                div { class: "absolute -top-15 left-1/2 -translate-x-1/2 text-xs text-primary font-mono bg-primary/10 px-1 rounded-md text-center whitespace-nowrap",
                    "{format_time(loop_start() as u32)}"
                }
                span { class: "material-symbols-outlined text-primary/90 text-[48px] hover:scale-115", "drag_indicator" }
            }

            // Правый маркер
            div {
                class: "absolute top-1/2 -translate-y-1/2 -translate-x-1/2 cursor-w-resize z-20",
                style: "left: {rp}%;",
                onmousedown: move |e| {
                    e.stop_propagation();
                    dragging.set(Some("R"));
                    let left = cl();
                    anchor_x.set(e.data().client_coordinates().x as f32 - left);
                    anchor_val.set(loop_end());
                },
                // Метка над правым маркером
                div { class: "absolute -top-15 left-1/2 -translate-x-1/2 text-xs text-primary font-mono bg-primary/10 px-1 rounded-md text-center whitespace-nowrap",
                    "{format_time(loop_end() as u32)}"
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
