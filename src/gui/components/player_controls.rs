use super::favorites::FavoritesButton;
use super::filters_spatial::FiltersAndSpatial;

use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::time::Duration;

#[component]
pub fn PlayerControls() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut position = use_signal(|| 0.0);
    let mut duration = use_signal(|| Duration::from_secs(0));
    let mut is_playing = use_signal(|| false);
    let mut repeat_enabled = use_signal(|| ctx.config.read().audio.repeat_track);
    let mut shuffle_enabled = use_signal(|| false);
    // let mut favorite_enabled = use_signal(|| false);

    // Слушаем статусы
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started {
                        duration: d,
                        metadata: _,
                    } => {
                        duration.set(d);
                        position.set(0.0);
                        is_playing.set(true);
                        println!("DEBUG EVENT: PlayerEvent::Started");
                    }
                    PlayerEvent::Paused => {
                        is_playing.set(false);
                        println!("DEBUG EVENT: PlayerEvent::Paused");
                    }
                    PlayerEvent::Resumed => {
                        is_playing.set(true);
                        println!("DEBUG EVENT: PlayerEvent::Resumed");
                    }
                    PlayerEvent::Finished => {
                        is_playing.set(false);
                        position.set(0.0); // сброс на 0;
                        println!("DEBUG EVENT: PlayerEvent::Finished");
                    }
                    _ => {} // ignore other variants
                }
            }
        }
    });

    /* use_future(move || {
        let pos = ctx.playback_progress.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                position.set(pos.load(Ordering::Relaxed) as f32 / 1000.0);
            }
        }
    }); */

    rsx! {
        section { class: "mt-18 px-6 py-4 neumorph-raised rounded-full flex items-center gap-6 hover:shadow",
            div { class: "flex items-center gap-4",
                ControlIconButton { icon: if repeat_enabled() { "repeat_one" } else { "repeat" },
                    title: "repeat track",
                    active: repeat_enabled(),
                    click_handler: {
                        move |_| {
                            let is_track_repeated = !repeat_enabled();
                            let _ = ctx.command_tx.read().send(PlayerCommand::ToggleRepeatTrack(is_track_repeated));
                            repeat_enabled.set(is_track_repeated);
                        }
                    }}
                ControlIconButton { icon: if shuffle_enabled() { "shuffle_on" } else { "shuffle" },
                    title: "shuffle tracks",
                    active: shuffle_enabled(),
                    click_handler: {
                        move |_| {
                            shuffle_enabled.set(!shuffle_enabled());
                        }
                    }
                }
                FavoritesButton {}
                /* ControlIconButton { icon: "favorite", title: "add to favorites",
                    active: favorite_enabled(),
                    click_handler: {
                        move |_| {
                            favorite_enabled.set(!favorite_enabled());
                        }
                    }
                } */
            }

            div { class: "h-6 w-[1px] bg-outline-variant opacity-50" }

            div { class: "flex items-center gap-4",

                // Prev track button
                button {
                    class: "w-10 h-10 rounded-full flex items-center justify-center text-secondary hover:bg-surface-container hover:text-primary cursor-pointer transition-all active:scale-90",
                    title: "previous track",
                    span { class: "material-symbols-outlined text-[24px]", "skip_previous" }
                }

                // Play/Pause/Resume button
                button {
                    class: "w-16 h-16 rounded-full flex items-center justify-center bg-primary text-on-primary shadow-lg transition-all cursor-pointer hover:scale-105 active:scale-95 group",
                    title: if is_playing() { "pause track" } else { "play track" },
                    onclick: {
                        move |_| {
                            if is_playing() {
                                let _ = ctx.command_tx.read().send(PlayerCommand::Pause);
                            } else {
                                let _ = ctx.command_tx.read().send(PlayerCommand::Resume);
                            }
                        }
                    },
                    span { class: "material-symbols-outlined text-[36px]",
                        style: "font-variation-settings: \"FILL\" 1;",
                        if is_playing() { "pause" } else { "play_arrow" }
                    }
                }

                // Next track button
                button {
                    class: "w-10 h-10 rounded-full flex items-center justify-center text-secondary hover:bg-surface-container cursor-pointer hover:text-primary transition-all active:scale-90",
                    title: "next track",
                    span { class: "material-symbols-outlined text-[24px]", "skip_next" }
                }
            }

            div { class: "h-6 w-[1px] bg-outline-variant opacity-50" }

            div { class: "flex items-center gap-3",
                SpeedControl {}
                FiltersAndSpatial {}
                VolumeControl {}
                // LUFS {}
            }
        }
    }
}

#[component]
pub fn ControlIconButton(
    icon: String,
    title: String,
    #[props(optional)] active: Option<bool>,
    #[props(optional)] click_handler: EventHandler<MouseEvent>,
) -> Element {
    let is_active = active.unwrap_or(false);

    rsx! {
        button {
            class: if is_active {
                "w-10 h-10 rounded-full flex items-center justify-center text-primary hover:bg-surface-container cursor-pointer transition-all active:scale-90"
            } else {
                "w-10 h-10 rounded-full flex items-center justify-center text-secondary hover:bg-surface-container cursor-pointer hover:text-primary transition-all active:scale-90"
            },
            title: "{title}",
            onclick: move |e| click_handler.call(e),
            span { class: "material-symbols-outlined text-[20px]",
                  style: if is_active {"font-variation-settings: 'FILL' 1;"} else {"font-variation-settings: 'FILL' 0;"},
                  "{icon}"
            }
        }
    }
}

#[component]
fn SpeedControl() -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut speed = use_signal(|| ctx.config.read().audio.speed as f32);
    let mut is_preserve_pitch = use_signal(|| ctx.config.read().audio.preserve_pitch);

    let mut show_speed_popup = use_signal(|| false);

    let speeds = [
        "0.25x", "0.5x", "0.75x", "1.0x", "1.25x", "1.5x", "1.75x", "2.0x",
    ];
    let speed_vals = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

    rsx! {
        div { class: "relative",
            button { class: "w-16 px-2 py-1 rounded-md text-label-md font-bold text-secondary hover:bg-surface-container cursor-pointer hover:text-primary transition-colors",
                title: "Change track speed",
                onclick: move |_| show_speed_popup.set(!show_speed_popup()),
                // onmouseenter: move |_| show_speed_popup.set(true),
                "{speed():.2}x"
            }
            if show_speed_popup() {
                // close popup by outside click
                div {
                    class: "fixed inset-0 z-20",
                    onclick: move |_| show_speed_popup.set(false),
                }

                div {
                    class: "absolute bottom-full left-1/2 -translate-x-1/2 mb-2 z-30",

                    div {
                        class: "neumorph-raised rounded-xl p-2 flex flex-col gap-1 min-w-[60px]",

                        onmouseenter: move |_| show_speed_popup.set(true),
                        // onmouseleave: move |_| show_speed_popup.set(false),

                        // Speed list and vertical slider
                        div {
                            class: "flex flex-row items-stretch gap-2",

                            // Original speed list
                            div {
                                class: "flex flex-col gap-1 min-w-[60px]",

                                {speeds.iter().enumerate().map(|(i, label)| {
                                    let val = speed_vals[i];
                                    let active = (speed() - val as f32).abs() < 0.01;

                                    rsx! {
                                        button {
                                            class: if active {
                                                "px-2 py-1 rounded-md text-label-sm bg-primary/10 text-primary cursor-pointer font-bold"
                                            } else {
                                                "px-2 py-1 rounded-md text-label-sm hover:bg-primary/10 hover:text-primary cursor-pointer text-secondary transition-colors"
                                            },

                                            onclick: move |_| {
                                                if ctx
                                                    .command_tx
                                                    .read()
                                                    .send(PlayerCommand::SetSpeed(val))
                                                    .is_ok()
                                                {
                                                    speed.set(val);
                                                    show_speed_popup.set(false);
                                                }
                                            },

                                            "{label}"
                                        }
                                    }
                                })}
                            }

                            // Vertical slider aligned to the full speed list height
                            div {
                                class: "relative flex w-6 self-stretch shrink-0 items-center justify-center border-l border-outline-variant/50",

                                input {
                                    r#type: "range",
                                    min: "0.25",
                                    max: "2.0",
                                    step: "0.01",
                                    value: "{speed()}",
                                    title: "Speed: {speed():.2}x",

                                    class: "absolute left-1/2 top-1/2 h-2 shrink-0 cursor-pointer accent-primary",

                                    // The slider height follows the speed list.
                                    style: "left: calc(50% + 2px); width: 280px; transform: translate(-50%, -50%) rotate(90deg); transform-origin: center;",

                                    oninput: move |event| {
                                        let Ok(value) = event.value().parse::<f32>() else {
                                            return;
                                        };

                                        let value = value.clamp(0.25, 2.0);

                                        if ctx
                                            .command_tx
                                            .read()
                                            .send(PlayerCommand::SetSpeed(value))
                                            .is_ok()
                                        {
                                            speed.set(value);
                                        }
                                    }
                                }
                            }

                        }

                        // Original separator
                        div {
                            class: "w-full h-[1px] bg-outline-variant/50 my-1",
                        }

                        // Original Preserve Pitch block
                        label {
                            class: "px-2 py-1.5 rounded-md text-[12px] text-label-sm flex items-center gap-2.5 hover:bg-primary/10 hover:text-primary cursor-pointer transition-colors w-full select-none whitespace-nowrap",

                            input {
                                r#type: "checkbox",
                                class: "sr-only",
                                checked: is_preserve_pitch(),

                                onchange: move |_| {
                                    let enabled = !is_preserve_pitch();
                                    // let enabled = evt.checked();

                                    if ctx
                                        .command_tx
                                        .read()
                                        .send(PlayerCommand::SetPreservePitch(enabled))
                                        .is_ok()
                                    {
                                        is_preserve_pitch.set(enabled);
                                    }
                                }
                            }

                            div {
                                class: format!(
                                    "w-4 h-4 border rounded-md flex items-center justify-center transition-all duration-150 shrink-0 {}",
                                    if is_preserve_pitch() {
                                        "bg-primary border-primary text-white"
                                    } else {
                                        "border-neutral-300 bg-transparent"
                                    }
                                ),

                                if is_preserve_pitch() {
                                    svg {
                                        class: "w-2.5 h-2.5 stroke-current",
                                        fill: "none",
                                        view_box: "0 0 24 24",
                                        stroke_width: "3",
                                        stroke_linecap: "round",
                                        stroke_linejoin: "round",

                                        path {
                                            d: "M20 6L9 17l-5-5"
                                        }
                                    }
                                }
                            }

                            span {
                                class: format!(
                                    "transition-colors {}",
                                    if is_preserve_pitch() {
                                        "text-primary font-medium"
                                    } else {
                                        "text-secondary"
                                    }
                                ),

                                "Preserve"
                                br {}
                                "Pitch"
                            }
                        }
                    }
                }
            }

        }
    }
}

#[component]
fn VolumeControl() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut volume = use_signal(|| ctx.config.read().audio.volume as f32);

    rsx! {
        div { class: "relative group/volume",
            button { class: "text-secondary hover:bg-surface-container hover:text-primary transition-colors flex items-center",
                title: "change track volume",
                /* onclick: move |_| {
                    let new_vol = if vol > 0.0 { 0.0 } else { 0.75 };
                    ctx.command_tx.send(PlayerCommand::SetVolume(new_vol));
                },
                span { class: "material-symbols-outlined text-[20px] text-secondary",
                    if vol > 0.5 { "volume_up" } else if vol > 0.0 { "volume_down" } else { "volume_off" }
                }
                div { class: "relative w-24 h-6 flex items-center",
                    div { class: "w-full h-1.5 bg-surface-container-highest rounded-full neumorph-sunken relative",
                        div { class: "absolute left-0 top-0 h-full bg-primary rounded-full",
                            style: "width: {vol_pct}%",
                        }
                        div { class: "absolute top-1/2 -translate-y-1/2 w-4 h-4 bg-primary rounded-full shadow-lg shadow-primary/20 border-2 border-white cursor-pointer hover:scale-110 transition-transform",
                            style: "left: {vol_pct}%; transform: translate(-50%, -50%);",
                        }
                    }
                } */
                span { class: "material-symbols-outlined text-[20px] text-secondary",
                    if volume() > 0.5 { "volume_up" } else if volume() > 0.0 { "volume_down" } else { "volume_off" }
                }
                input {
                    class: "w-24 h-1.5 accent-primary cursor-pointer",
                    r#type: "range",
                    min: 0.0,
                    max: 1.0,
                    step: "0.01",
                    value: "{volume}",
                    oninput: move |e| {
                        let current_volume: f32 = e.value().parse().unwrap_or(0.0);
                        // TODO: atomic value?
                        let _ = ctx.command_tx.read().send(PlayerCommand::SetVolume(current_volume));
                        volume.set(current_volume);
                    },
                }
                span { class: "text-label-sm font-bold text-secondary w-10 text-right whitespace-nowrap", "{(volume*100.0):.0}%" }
            }
        }
    }
}

#[component]
fn LUFS() -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut lufs = use_signal(|| None::<f32>);

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();

        async move {
            while let Some(event) = status_rx.next().await {
                match event {
                    PlayerEvent::Started { .. } => {
                        lufs.set(None);
                    }

                    PlayerEvent::LoudnessMeasured {
                        momentary_lufs,
                        shortterm_lufs,
                        integrated_lufs: _,
                    } => {
                        lufs.set(shortterm_lufs.or(momentary_lufs));
                    }

                    _ => {}
                }
            }
        }
    });

    rsx! {
        span {
            class: "text-label-sm font-bold text-secondary whitespace-nowrap",
            match lufs() {
                Some(value) => format!("{value:.1} LUFS"),
                None => "— LUFS".to_string(),
            }
        }

    }
}
