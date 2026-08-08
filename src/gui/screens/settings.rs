use dioxus::prelude::*;

#[component]
pub fn Settings() -> Element {
    let mut is_cursor_checked = use_signal(|| false);
    let mut is_time_markers_checked = use_signal(|| true);
    let mut is_loop_enabled = use_signal(|| true);
    let mut is_standart_timeline_view = use_signal(|| false);
    rsx! {
        h1 { "Settings screen!" }
        SettingsCheckbox {
            id: "timeline-cursor-behavior",
            text: "Enable timeline cursor followed by loop movement.",
            checked: is_cursor_checked,
            onchange: move |_| is_cursor_checked.set(!is_cursor_checked()),
        }
        SettingsCheckbox {
            id: "time-markers-visability",
            text: "Show time markers below the timeline.",
            checked: is_time_markers_checked,
            onchange: move |_| is_time_markers_checked.set(!is_time_markers_checked()),
        }
        SettingsCheckbox {
            id: "enable-loop",
            text: "Enable loop repeat region.",
            checked: is_loop_enabled,
            onchange: move |_| is_loop_enabled.set(!is_loop_enabled()),
        }
        SettingsCheckbox {
            id: "timeline-progress-view",
            text: "Change timeline standart progress view with round cursor.",
            checked: is_standart_timeline_view,
            onchange: move |_| is_standart_timeline_view.set(!is_standart_timeline_view()),
        }

        // Shortcuts
        div { class: "bg-surface-container-lowest p-6 rounded-2xl shadow-[0_4px_20px_rgba(107,122,143,0.06)] w-full mb-8",
            // Заголовок с иконкой: отступ снизу mb-5 и расстояние между иконкой и текстом gap-2
            div { class: "flex items-center gap-2 mb-5",
                span { class: "material-symbols-outlined text-primary text-xl", "keyboard" }
                h4 { class: "text-lg font-semibold text-primary tracking-tight", "Shortcuts" }
            }

            // Сетка шорткатов: gap-x-16 разделяет колонки, gap-y-3 дает вертикальный зазор
            div { class: "grid grid-cols-1 md:grid-cols-2 gap-x-16 gap-y-3 w-full",

                // Play / Pause
                div { class: "flex justify-between items-center py-2.5 border-b border-surface-container w-full",
                    span { class: "text-sm text-on-surface font-medium", "Play / Pause" }
                    kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "SPACE" }
                }

                // Rewind Forward +30
                div { class: "flex justify-between items-center py-2.5 border-b border-surface-container w-full",
                    span { class: "text-sm text-on-surface font-medium", "Rewind Forward +30 sec." }
                    kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "CMD + →" }
                }

                // Rewind Backward -30
                div { class: "flex justify-between items-center py-2.5 border-b border-surface-container w-full",
                    span { class: "text-sm text-on-surface font-medium", "Rewind Backward -30 sec." }
                    kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "CMD + ←" }
                }

                // Rewind Forward +5
                div { class: "flex justify-between items-center py-2.5 border-b border-surface-container w-full",
                    span { class: "text-sm text-on-surface font-medium", "Rewind Forward +5 sec." }
                    kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "→" }
                }

                // Rewind Backward -5
                div { class: "flex justify-between items-center py-2.5 border-b border-surface-container w-full",
                    span { class: "text-sm text-on-surface font-medium", "Rewind Backward -5 sec." }
                    kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "←" }
                }

                // Wheel zoom waveform
                div { class: "flex justify-between items-center py-2.5 border-b border-surface-container w-full",
                    span { class: "text-sm text-on-surface font-medium", "Zoom waveform" }
                    kbd { class: "px-2 py-1 bg-surface-container-low rounded border border-outline-variant text-[10px] font-mono text-outline tracking-wider shadow-sm", "Wheel Up / Wheel Down" }
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
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        label {
            id: "{id}",
            class: format!(
                    "rounded-md text-[16px] text-label-md flex items-center hover:bg-primary/10 hover:text-primary cursor-pointer transition-colors select-none whitespace-nowrap {}",
                    if text.is_empty() {
                        // Квадратный контейнер и строгая центровка для иконки без текста
                        "w-8 h-8 justify-center"
                    } else {
                        // Стандартные отступы и выравнивание по левому краю для чекбоксов с текстом
                        "px-2 py-1.5 w-full gap-2.5 justify-start"
                    }
            ),

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
                onchange: move |e| onchange.call(e.value()),
            }

            if !text.is_empty() {
                    span { "{text}" }
            }
        }
    }
}
