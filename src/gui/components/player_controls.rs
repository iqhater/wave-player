use super::favorites::FavoritesButton;
use super::filters_spatial::FiltersAndSpatial;

use crate::PlayerContext;
use crate::gui::layout::PlaybackUIState;
use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::PlayerEvent;

use dioxus::prelude::*;
use futures_util::StreamExt;
use std::time::Duration;

#[component]
pub fn PlayerControls() -> Element {
    let ctx = use_context::<PlayerContext>();
    let playback = use_context::<PlaybackUIState>();

    let mut position = use_signal(|| 0.0);
    let mut duration = use_signal(|| Duration::from_secs(0));

    // Общее состояние RootLayout: переживает размонтирование PlayerControls.
    let mut is_playing = playback.is_playing;

    let mut repeat_enabled = use_signal(|| ctx.config.read().audio.repeat_track);
    let mut has_next = use_signal(|| false);
    let mut shuffle_enabled = use_signal(|| false);

    // Слушаем статусы
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            let _ = ctx.command_tx.read().send(PlayerCommand::RequestState);

            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started {
                        duration: d,
                        metadata: _,
                    } => {
                        duration.set(d);
                        position.set(0.0);
                        is_playing.set(true);
                    }
                    PlayerEvent::Paused => {
                        is_playing.set(false);
                    }
                    PlayerEvent::Resumed => {
                        is_playing.set(true);
                    }
                    PlayerEvent::Finished => {
                        is_playing.set(false);
                        position.set(0.0); // сброс на 0;
                    }
                    PlayerEvent::PlayerState {
                        is_playing: playing,
                        ..
                    } => {
                        is_playing.set(playing);
                    }
                    PlayerEvent::QueueChanged {
                        has_next: next_available,
                        shuffle,
                        ..
                    } => {
                        has_next.set(next_available);
                        shuffle_enabled.set(shuffle);
                    }
                    _ => {} // ignore other variants
                }
            }
        }
    });

    rsx! {
        section { class: "fixed bottom-0 left-0 right-0 z-40
                    flex min-h-[110px] w-full items-center justify-center
                    gap-1 px-3 py-6
                    neumorph-raised
                    md:min-h-[130px]
                    lg:static lg:min-h-0 lg:w-auto
                    lg:gap-6 lg:rounded-full lg:px-6 lg:py-4
                    hover:shadow",
            div { class: "flex shrink-0 items-center gap-4
                          md:gap-6
                          lg:gap-4",
                ControlIconButton { icon: if repeat_enabled() { "repeat_one" } else { "repeat" },
                    title: "repeat track",
                    active: repeat_enabled(),
                    click_handler: {
                        move |_| {
                            let is_track_repeated = !repeat_enabled();

                            let _ = ctx.command_tx.read().send(PlayerCommand::ToggleRepeatTrack(is_track_repeated)).is_ok();
                            {
                                repeat_enabled.set(is_track_repeated);
                                ctx.update_config(|config| {
                                    config.audio.repeat_track = is_track_repeated;
                                });
                            }
                        }
                    }}
                div {
                    class: "hidden lg:block",

                    ControlIconButton { icon: if shuffle_enabled() { "shuffle_on" } else { "shuffle" },
                        title: "shuffle tracks",
                        active: shuffle_enabled(),
                        click_handler: {
                            move |_| {
                                let enabled = !shuffle_enabled();

                                if ctx
                                    .command_tx
                                    .read()
                                    .send(PlayerCommand::SetShuffle(enabled))
                                    .is_err()
                                {
                                    return;
                                }

                                shuffle_enabled.set(enabled);
                            }
                        }
                    }
                }
                FavoritesButton {}
            }

            // Separator
            div { class: "hidden h-6 w-[1px] bg-outline-variant opacity-50 lg:block" }

            div { class: "flex shrink-0 items-center gap-4
                          md:gap-6
                          lg:gap-4",

                // Prev track button
                button {
                    class: "w-10 h-10 rounded-full flex items-center justify-center text-secondary hover:bg-surface-container hover:text-primary cursor-pointer transition-all active:scale-90",
                    title: "previous track",
                    onclick: move |_| {
                        let _ = ctx
                            .command_tx
                            .read()
                            .send(PlayerCommand::PreviousTrack);
                    },
                    span { class: "material-symbols-outlined text-[24px]", "skip_previous" }
                }

                // Play/Pause/Resume button
                button {
                    class: "flex h-14 w-14 items-center justify-center
                            rounded-full bg-primary text-on-primary shadow-lg
                            transition-all cursor-pointer
                            hover:scale-105 active:scale-95
                            lg:h-16 lg:w-16 group",
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
                    span { class: "material-symbols-outlined text-[30px] lg:text-[36px]",
                        style: "font-variation-settings: \"FILL\" 1;",
                        if is_playing() { "pause" } else { "play_arrow" }
                    }
                }

                // Next track button
                button {
                    class: "w-10 h-10 rounded-full flex items-center justify-center text-secondary hover:bg-surface-container cursor-pointer hover:text-primary transition-all active:scale-90",
                    title: "next track",
                    disabled: !has_next(),
                    onclick: move |_| {
                        let _ = ctx
                            .command_tx
                            .read()
                            .send(PlayerCommand::NextTrack);
                    },
                    span { class: "material-symbols-outlined text-[24px]", "skip_next" }
                }
            }

            // Separator
            div { class: "hidden h-6 w-[1px] bg-outline-variant opacity-50 lg:block" }

            div { class: "flex items-center gap-1 lg:gap-3",
                SpeedControl {}

                div {
                    class: "hidden lg:block",
                    FiltersAndSpatial {}
                }

                VolumeControl {}
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
                "flex h-9 w-9 items-center justify-center rounded-full
                 text-primary hover:bg-surface-container
                 cursor-pointer transition-all active:scale-90
                 lg:h-10 lg:w-10"
            } else {
                "flex h-9 w-9 items-center justify-center rounded-full
                 text-secondary hover:bg-surface-container
                 cursor-pointer hover:text-primary
                 transition-all active:scale-90
                 lg:h-10 lg:w-10"
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
    let playback = use_context::<PlaybackUIState>();

    // Состояние принадлежит RootLayout и не сбрасывается
    // при пересоздании PlayerControls.
    let mut speed = playback.speed;
    let mut is_preserve_pitch = playback.preserve_pitch;

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

                                                    ctx.update_config(|config| {
                                                        config.audio.speed = val;
                                                    });
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
                                    },

                                    onchange: move |_| {
                                        let value = speed();
                                        ctx.update_config(|config| {
                                            config.audio.speed = value;
                                        });
                                    },
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

                                    if ctx
                                        .command_tx
                                        .read()
                                        .send(PlayerCommand::SetPreservePitch(enabled))
                                        .is_ok()
                                    {
                                        is_preserve_pitch.set(enabled);

                                        ctx.update_config(|config| {
                                            config.audio.preserve_pitch = enabled;
                                        });
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
    let mut show_volume_slider = use_signal(|| false);

    rsx! {
        div {
            class: "relative flex items-center",

            // Desktop horizontal volume control
            div {
                class: "hidden items-center lg:flex",

                span {
                    class: "material-symbols-outlined text-[20px] text-secondary",
                    title: "change volume",

                    if volume() > 0.5 {
                        "volume_up"
                    } else if volume() > 0.0 {
                        "volume_down"
                    } else {
                        "volume_off"
                    }
                }

                div {
                        class: "relative flex items-center",

                        input {
                            class: "mx-2 h-1.5 w-24 accent-primary cursor-pointer",
                            r#type: "range",
                            min: "0.0",
                            max: "1.0",
                            step: "0.01",
                            value: "{volume()}",

                            oninput: move |event| {
                                let Ok(current_volume) = event.value().parse::<f32>() else {
                                    return;
                                };

                                let current_volume = current_volume.clamp(0.0, 1.0);

                                let _ = ctx
                                    .command_tx
                                    .read()
                                    .send(PlayerCommand::SetVolume(current_volume));

                                volume.set(current_volume);
                            },

                            onchange: move |_| {
                                let value = volume();
                                ctx.update_config(|config| {
                                    config.audio.volume = value;
                                });
                            },
                        }

                        div {
                            class: "absolute bottom-full left-1/2
                                    mb-2 -translate-x-1/2 whitespace-nowrap",

                            LUFS {}
                        }
                }

                span {
                    class: "w-10 text-right text-label-sm
                            font-bold text-secondary whitespace-nowrap",

                    "{(volume() * 100.0):.0}%"
                }
            }

            // Mobile/tablet volume button
            div {
                class: "relative lg:hidden",

                div {
                    class: "absolute bottom-full left-1/2
                            mb-0 -translate-x-1/2 whitespace-nowrap",

                    LUFS {}
                }

                button {
                    class: "relative z-50 flex h-10 w-10 items-center
                            justify-center rounded-full text-secondary
                            hover:bg-surface-container
                            hover:text-primary
                            cursor-pointer transition-all
                            active:scale-90",

                    title: "Volume: {(volume() * 100.0) as u32}%",
                    "aria-label": "Volume: {(volume() * 100.0) as u32}%",

                    onclick: move |_| {
                        show_volume_slider.toggle();
                    },

                    onchange: move |_| {
                        let value = volume();
                        ctx.update_config(|config| {
                            config.audio.volume = value;
                        });
                    },

                    span {
                        class: "material-symbols-outlined text-[24px]",

                        if volume() > 0.5 {
                            "volume_up"
                        } else if volume() > 0.0 {
                            "volume_down"
                        } else {
                            "volume_off"
                        }
                    }
                }

                // Vertical volume slider popup
                if show_volume_slider() {
                    // Закрывает popup при клике вне регулятора
                    div {
                        class: "fixed inset-0 z-40 lg:hidden",

                        onclick: move |_| {
                            show_volume_slider.set(false);
                        },
                    }

                    // Popup регулятора громкости
                    div {
                        class: "absolute bottom-full right-0 z-50 mb-1
                                flex h-36 w-10 flex-col items-center justify-center
                                gap-1 rounded-xl bg-surface-container-high
                                p-1.5 shadow-lg",

                        span {
                            class: "text-xs font-bold text-secondary
                                    whitespace-nowrap",

                            "{(volume() * 100.0):.0}%"
                        }

                        input {
                            class: "h-24 w-1 accent-primary cursor-pointer",

                            r#type: "range",
                            min: "0.0",
                            max: "1.0",
                            step: "0.01",
                            value: "{volume()}",

                            style: "writing-mode: vertical-lr; direction: rtl;",

                            title: "Change track volume",

                            oninput: move |event| {
                                let Ok(current_volume) = event.value().parse::<f32>() else {
                                    return;
                                };

                                let current_volume = current_volume.clamp(0.0, 1.0);

                                let _ = ctx
                                    .command_tx
                                    .read()
                                    .send(PlayerCommand::SetVolume(current_volume));

                                volume.set(current_volume);
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LUFS() -> Element {
    let playback = use_context::<PlaybackUIState>();
    let lufs = playback.integrated_lufs;

    rsx! {
        span {
            class: "text-[12px] font-bold text-primary whitespace-nowrap",
            title: "Track LUFS Integrated",
            match lufs() {
                Some(value) => format!("{value:.1} LUFS-I"),
                None => "— LUFS-I".to_string(),
            }
        }

    }
}

/// Keeps the last valid integrated loudness value when an event carries a
/// missing or unusable measurement.
pub(crate) fn accepted_integrated_lufs(current: Option<f32>, event: Option<f32>) -> Option<f32> {
    match event {
        Some(value) if value.is_finite() && (-60.0..=0.0).contains(&value) => Some(value),
        _ => current,
    }
}

#[cfg(test)]
mod lufs_tests {
    use super::accepted_integrated_lufs;

    #[test]
    fn valid_value_should_replace_the_current_one() {
        assert_eq!(
            accepted_integrated_lufs(Some(-18.0), Some(-14.0)),
            Some(-14.0)
        );
        assert_eq!(accepted_integrated_lufs(None, Some(-14.0)), Some(-14.0));
    }

    #[test]
    fn missing_or_out_of_range_value_should_keep_the_current_one() {
        assert_eq!(accepted_integrated_lufs(Some(-14.0), None), Some(-14.0));
        assert_eq!(
            accepted_integrated_lufs(Some(-14.0), Some(-80.0)),
            Some(-14.0)
        );
        assert_eq!(
            accepted_integrated_lufs(Some(-14.0), Some(5.0)),
            Some(-14.0)
        );
        assert_eq!(accepted_integrated_lufs(None, None), None);
    }

    #[test]
    fn boundary_values_should_be_accepted() {
        assert_eq!(accepted_integrated_lufs(None, Some(-60.0)), Some(-60.0));
        assert_eq!(accepted_integrated_lufs(None, Some(0.0)), Some(0.0));
    }

    #[test]
    fn non_finite_values_should_be_rejected() {
        let current = Some(-14.0);

        assert_eq!(accepted_integrated_lufs(current, Some(f32::NAN)), current);
        assert_eq!(
            accepted_integrated_lufs(current, Some(f32::INFINITY)),
            current
        );
        assert_eq!(
            accepted_integrated_lufs(current, Some(f32::NEG_INFINITY)),
            current
        );
    }

    #[test]
    fn track_reset_then_measurement_should_restore_a_value() {
        // `Started` clears the value; a later measurement fills it again.
        let after_start = None;
        let after_measurement = accepted_integrated_lufs(after_start, Some(-9.5));

        assert_eq!(after_measurement, Some(-9.5));
    }
}
