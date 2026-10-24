use super::super::layout::{
    BeatGridSnap, LoopRepeatRegion, PlaybackUIState, ShowBeatGrid, ShowTimeMarkers,
    TimelineCursorFollowsLoop, TimelineView,
};
use crate::PlayerContext;
use crate::shared::command_types::{PlayerCommand, StoreCommand};
use dioxus::prelude::*;

const MIN_PREAMP_DB: f32 = -24.0;
const MAX_PREAMP_DB: f32 = 24.0;
const PREAMP_STEP_DB: f32 = 0.5;

fn adjust_preamp(ctx: PlayerContext, mut preamp_db: Signal<f32>, delta: f32) {
    let value = (preamp_db() + delta).clamp(MIN_PREAMP_DB, MAX_PREAMP_DB);

    if (value - preamp_db()).abs() < f32::EPSILON {
        return;
    }

    preamp_db.set(value);

    let _ = ctx.command_tx.read().send(PlayerCommand::SetPreamp(value));

    ctx.update_config(|config| {
        config.audio.preamp_db = value;
    });
}

#[component]
pub fn Settings() -> Element {
    let ctx = use_context::<PlayerContext>();

    // Общие настройки из RootLayout.

    // Grid
    let mut is_grid_enabled = use_context::<ShowBeatGrid>().0;
    let mut is_grid_snap = use_context::<BeatGridSnap>().0;

    // Timeline & loop
    let mut is_cursor_checked = use_context::<TimelineCursorFollowsLoop>().0;
    let mut is_time_markers_checked = use_context::<ShowTimeMarkers>().0;
    let mut is_loop_enabled = use_context::<LoopRepeatRegion>().0;
    let mut is_standard_timeline_view = use_context::<TimelineView>().0;

    // Audio
    let has_track = use_context::<PlaybackUIState>().has_track;
    let preamp_db = use_signal(|| ctx.config.read().audio.preamp_db);

    rsx! {
        div {
            class: "w-full flex flex-col gap-6",

            h1 {
                class: "text-2xl font-semibold text-secondary",
                "Settings"
            }

             // Timeline settings
            div {
                class: "w-full rounded-2xl bg-surface-container-lowest
                                p-5 shadow-[0_4px_20px_rgba(107,122,143,0.06)]",

                div {
                    class: "flex items-center gap-2 mb-4",

                    span {
                        class: "material-symbols-outlined text-primary",
                        "timeline"
                    }

                    h2 {
                        class: "text-lg font-semibold text-primary",
                        "Timeline"
                    }
                }

                div {
                    class: "flex flex-col gap-2",

                    SettingsCheckbox {
                        id: "timeline-cursor-behavior",
                        text: "Enable timeline cursor followed by loop movement.",
                        checked: is_cursor_checked,
                        disabled: true,
                        onchange: move |_| {
                            let enabled = !is_cursor_checked();
                            is_cursor_checked.set(enabled);

                            ctx.update_config(|config| {
                                config.ui.timeline_cursor_follows_loop = enabled;
                            });
                        },
                    }

                    SettingsCheckbox {
                        id: "time-markers-visability",
                        text: "Show time markers below the timeline.",
                        checked: is_time_markers_checked,
                        onchange: move |_| {
                            let enabled = !is_time_markers_checked();
                            is_time_markers_checked.set(enabled);

                            let mut config = ctx.config.read().clone();
                            config.ui.show_time_markers = enabled;

                            *ctx.config.write() = config.clone();

                            let _ = ctx
                                .store_cmd_tx
                                .read()
                                .send(StoreCommand::SetConfig(config));
                        },
                    }

                    SettingsCheckbox {
                        id: "enable-loop",
                        text: "Enable loop region repeat.",
                        checked: is_loop_enabled,
                        onchange: move |_| {
                            let enabled = !is_loop_enabled();
                            is_loop_enabled.set(enabled);

                            let mut config = ctx.config.read().clone();
                            config.ui.loop_repeat_region = enabled;

                            *ctx.config.write() = config.clone();

                            let _ = ctx
                                .store_cmd_tx
                                .read()
                                .send(StoreCommand::SetConfig(config));
                        },
                    }

                    SettingsCheckbox {
                        id: "timeline-progress-view",
                        text: "Use the round cursor progress view.",
                        checked: is_standard_timeline_view,
                        disabled: true,
                        onchange: move |_| {
                            let enabled = !is_standard_timeline_view();
                            is_standard_timeline_view.set(enabled);

                            ctx.update_config(|config| {
                                config.ui.timeline_progress_view = enabled;
                            });
                        },
                    }
                }
            }

            // Beat grid settings
            div {
                class: "w-full rounded-2xl bg-surface-container-lowest
                        p-5 shadow-[0_4px_20px_rgba(107,122,143,0.06)]",

                div {
                    class: "flex items-center gap-2 mb-4",

                    span {
                        class: "material-symbols-outlined text-primary",
                        "grid_4x4"
                    }

                    h2 {
                        class: "text-lg font-semibold text-primary",
                        "Beat Grid"
                    }
                }

            div {
                class: "flex flex-col gap-2",

                SettingsCheckbox {
                    id: "beat-grid",
                    text: "Show the beat grid when BPM is available.",
                    checked: is_grid_enabled,
                    onchange: move |_| {
                        let enabled = !is_grid_enabled();
                        is_grid_enabled.set(enabled);

                        // Snap — отдельная настройка. Она просто не применяется,
                        // пока сетка выключена, и сохраняется при повторном включении.
                        let mut config = ctx.config.read().clone();
                        config.ui.show_beat_grid = enabled;
                        config.ui.beat_grid_snap = is_grid_snap();

                        *ctx.config.write() = config.clone();

                        let _ = ctx
                        .store_cmd_tx
                        .read()
                        .send(StoreCommand::SetConfig(config));
                    }
                }
            }
                SettingsCheckbox {
                    id: "beat-grid-snap",
                    text: "Snap timeline seeking to the beat grid.",
                    checked: is_grid_snap,
                    onchange: move |_| {

                        let enabled = !is_grid_snap();
                        is_grid_snap.set(enabled);

                        let mut config = ctx.config.read().clone();
                        config.ui.show_beat_grid = is_grid_enabled();
                        config.ui.beat_grid_snap = enabled;

                        *ctx.config.write() = config.clone();

                        let _ = ctx
                        .store_cmd_tx
                        .read()
                        .send(StoreCommand::SetConfig(config));
                    }
                }

                p {
                    class: "mt-3 text-sm text-secondary",
                    "Manual BPM and grid anchor are saved per track. Right-click the BPM field to reset them."
                }
            }

            // Audio settings
            div {
                class: "w-full rounded-2xl bg-surface-container-lowest
                        p-5 shadow-[0_4px_20px_rgba(107,122,143,0.06)]",

                div {
                    class: "flex items-center gap-2 mb-4",

                    span {
                        class: "material-symbols-outlined text-primary",
                        "tune"
                    }

                    h2 {
                        class: "text-lg font-semibold text-primary",
                        "Audio"
                    }
                }

                div {
                    class: "flex flex-col gap-4",

                    div {
                        class: "flex items-center justify-between gap-4",

                        span { class: "text-[16px] text-on-surface", "Preamp" }

                        div {
                            class: "flex items-stretch overflow-hidden rounded-lg
                                    border border-outline-variant bg-surface-container",

                            span {
                                class: "flex w-20 items-center justify-center px-2 text-sm
                                        font-semibold text-primary tabular-nums",
                                {format!("{:+.1} dB", preamp_db())}
                            }

                            div {
                                class: "flex flex-col border-l border-outline-variant",

                                button {
                                    class: "flex h-5 w-7 cursor-pointer items-center justify-center
                                            text-secondary transition-colors
                                            hover:bg-primary/10 hover:text-primary
                                            active:bg-primary/20",
                                    r#type: "button",
                                    title: "Increase preamp",
                                    onclick: move |_| adjust_preamp(ctx, preamp_db, PREAMP_STEP_DB),

                                    span {
                                        class: "material-symbols-outlined",
                                        style: "font-size: 14px; line-height: 1;",
                                        "keyboard_arrow_up"
                                    }
                                }

                                button {
                                    class: "flex h-5 w-7 cursor-pointer items-center justify-center
                                            border-t border-outline-variant text-secondary
                                            transition-colors hover:bg-primary/10
                                            hover:text-primary active:bg-primary/20",
                                    r#type: "button",
                                    title: "Decrease preamp",
                                    onclick: move |_| adjust_preamp(ctx, preamp_db, -PREAMP_STEP_DB),

                                    span {
                                        class: "material-symbols-outlined",
                                        style: "font-size: 14px; line-height: 1;",
                                        "keyboard_arrow_down"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Analysis utilities
            div {
                class: "w-full rounded-2xl bg-surface-container-lowest
                        p-5 shadow-[0_4px_20px_rgba(107,122,143,0.06)]",

                div {
                    class: "flex items-center gap-2 mb-4",

                    span {
                        class: "material-symbols-outlined text-primary",
                        "analytics"
                    }

                    h2 {
                        class: "text-lg font-semibold text-primary",
                        "Analysis"
                    }
                }

                p {
                    class: "mb-4 text-sm text-secondary",
                    "Re-run beat detection for the current track or drop the cached analysis and waveforms for every track."
                }

                div {
                    class: "flex flex-wrap gap-3",

                    button {
                        class: "flex cursor-pointer items-center gap-2 rounded-xl bg-primary px-3 py-2
                                text-sm text-on-primary transition-all hover:brightness-110
                                active:scale-95 disabled:cursor-not-allowed disabled:opacity-50",
                        disabled: !has_track(),
                        title: "Ignore cached results and analyze the current track again",
                        onclick: move |_| {
                            let _ = ctx
                                .command_tx
                                .read()
                                .send(PlayerCommand::ReanalyzeCurrent);
                        },

                        span {
                            class: "material-symbols-outlined",
                            style: "font-size: 18px;",
                            "refresh"
                        }
                        span { "Reanalyze track" }
                    }

                    button {
                        class: "flex cursor-pointer items-center gap-2 rounded-xl bg-surface-container
                                px-3 py-2 text-sm text-secondary transition-all
                                hover:bg-surface-variant hover:text-primary active:scale-95",
                        title: "Delete cached analysis, BPM and waveform data for all tracks",
                        onclick: move |_| {
                            let _ = ctx
                                .command_tx
                                .read()
                                .send(PlayerCommand::ClearAnalysisCache);
                        },

                        span {
                            class: "material-symbols-outlined",
                            style: "font-size: 18px;",
                            "delete_sweep"
                        }
                        span { "Clear analysis cache" }
                    }
                }
            }

            // Shortcuts
            div { class: "w-full rounded-2xl bg-surface-container-lowest
                          p-5 shadow-[0_4px_20px_rgba(107,122,143,0.06)]",

                // Заголовок с иконкой: отступ снизу mb-5 и расстояние между иконкой и текстом gap-2
                div { class: "flex items-center gap-2 mb-5",
                    span { class: "material-symbols-outlined text-primary", "keyboard" }
                    h2 { class: "text-lg font-semibold text-primary tracking-tight", "Shortcuts" }
                }

                // Сетка шорткатов: gap-x-16 разделяет колонки, gap-y-3 дает вертикальный зазор
                div { class: "grid grid-cols-1 md:grid-cols-2 gap-x-16 gap-y-3 w-full",

                    // Play / Pause
                    div { class: "flex justify-between items-start gap-4 py-2.5 border-b border-surface-container w-full",
                        span { class: "min-w-0 break-words text-sm text-on-surface font-medium", "Play / Pause" }
                        kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "SPACE" }
                    }

                    // Rewind Forward +30
                    div { class: "flex justify-between items-start gap-4 py-2.5 border-b border-surface-container w-full",
                        span { class: "min-w-0 break-words text-sm text-on-surface font-medium", "Rewind Forward +30 sec." }
                        kbd { class: "shrink-0 px-2 py-1 bg-surface-container-low
                                    rounded border border-outline-variant text-[10px]
                                    font-mono text-outline tracking-wider shadow-sm", "CMD + →" }
                    }

                    // Rewind Backward -30
                    div { class: "flex justify-between items-start gap-4 py-2.5 border-b border-surface-container w-full",
                        span { class: "min-w-0 break-words text-sm text-on-surface font-medium", "Rewind Backward -30 sec." }
                        kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "CMD + ←" }
                    }

                    // Rewind Forward +5
                    div { class: "flex justify-between items-start gap-4 py-2.5 border-b border-surface-container w-full",
                        span { class: "min-w-0 break-words text-sm text-on-surface font-medium", "Rewind Forward +5 sec." }
                        kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "→" }
                    }

                    // Rewind Backward -5
                    div { class: "flex justify-between items-start gap-4 py-2.5 border-b border-surface-container w-full",
                        span { class: "min-w-0 break-words text-sm text-on-surface font-medium", "Rewind Backward -5 sec." }
                        kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "←" }
                    }

                    // Wheel zoom waveform
                    div { class: "flex justify-between items-start gap-4 py-2.5 border-b border-surface-container w-full",
                        span { class: "min-w-0 break-words text-sm text-on-surface font-medium", "Zoom waveform" }
                        kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "Wheel Up / Wheel Down" }
                    }

                    // Double-click resets knob values. (filter, pan, etc...)
                    div { class: "flex justify-between items-start gap-4 py-2.5 border-b border-surface-container w-full",
                        span { class: "min-w-0 break-words text-sm text-on-surface font-medium", "Reset knob value" }
                        kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "Double-click" }
                    }
                }
            }
        }
    }
}

#[component]
pub fn SettingsCheckbox(
    id: String,
    text: String,
    checked: Signal<bool>,
    #[props(optional)] disabled: Option<bool>,
    onchange: EventHandler<String>,
) -> Element {
    let is_disabled = disabled.unwrap_or(false);

    rsx! {
        label {
            id: "{id}",
            class: format!(
                    "rounded-md text-[16px] text-label-md flex items-center
                     transition-colors select-none whitespace-normal {} {}",
                    if is_disabled {
                        "opacity-50 cursor-not-allowed"
                    } else {
                        "cursor-pointer hover:bg-primary/10 hover:text-primary"
                    },
                    if text.is_empty() {
                        // Квадратный контейнер и строгая центровка для иконки без текста
                        "w-8 h-8 justify-center"
                    } else {
                        // Стандартные отступы и выравнивание по левому краю для чекбоксов с текстом
                        "px-2 py-1.5 w-full gap-2.5 justify-start"
                    }
            ),
            aria_disabled: is_disabled,

            div {
                class: format!(
                    "w-4 h-4 border rounded-md flex items-center justify-center transition-all duration-150 shrink-0 {}",
                    if checked() {
                        "bg-primary border-primary text-white"
                    } else {
                        "border-neutral-300 bg-transparent"
                    }
                ),

                if checked() {
                    svg {
                        class: "w-2.5 h-2.5 stroke-current",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke_width: "3",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M20 6L9 17l-5-5" }
                    }
                }
            }

            input {
                class: "sr-only",
                r#type: "checkbox",
                checked: checked(),
                disabled: is_disabled,
                onchange: move |e| onchange.call(e.value()),
            }

            if !text.is_empty() {
                span {
                    class: "min-w-0 break-words leading-6",
                    "{text}"
                }
            }
        }
    }
}
