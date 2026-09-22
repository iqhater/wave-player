use super::ControlIconButton;
use crate::PlayerContext;
use crate::gui::screens::settings::SettingsCheckbox;
use crate::shared::command_types::PlayerCommand;
use dioxus::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[component]
pub fn FiltersAndSpatial() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut show_menu_popup = use_signal(|| false);
    let mut ms_mode = use_signal(|| "none".to_string()); //TODO: enum Signal<String> ("none", "mid", "side")
    let stereo_width = use_signal(|| ctx.config.read().audio.stereo_width); // Signal<f32>, диапазон 0.0 .. 2.0 (дефолт 1.0)
    let pan = use_signal(|| ctx.config.read().audio.pan); // Signal<f32>, диапазон -1.0 .. 1.0 (дефолт 0.0)
    let lp_cutoff = use_signal(|| 20000.0f32); // Signal<f32>, диапазон 20.0 .. 20000.0
    let hp_cutoff = use_signal(|| 20.0f32); // Signal<f32>, диапазон 20.0 .. 20000.0
    let mut lp_enabled = use_signal(|| false); // Signal<bool>
    let mut hp_enabled = use_signal(|| false); // Signal<bool>

    let pan_text = if pan() == 0.0 {
        "Center".to_string()
    } else if pan() < 0.0 {
        format!("L {:.0}%", pan().abs() * 100.0)
    } else {
        format!("R {:.0}%", pan() * 100.0)
    };

    rsx! {
        div { class: "relative",

            ControlIconButton { icon: "discover_tune", title: "spatial & filters tune", active: show_menu_popup(), click_handler: {
                move |_| show_menu_popup.set(!show_menu_popup())
            } }

            if show_menu_popup() {
                // close popup by outside click
                div {
                    class: "fixed inset-0 z-20",
                    onclick: move |_| show_menu_popup.set(false),
                }

                div { class: "absolute bottom-full left-1/2 -translate-x-1/2 mb-2 z-30",
                    div { class: "neumorph-raised rounded-xl p-2 flex flex-col gap-1 min-w-[140px]",
                        onmouseenter: move |_| show_menu_popup.set(true),
                        // onmouseleave: move |_| show_menu_popup.set(false),
                        // onpointercancel: move |_| show_menu_popup.set(false),

                        // --- СЕКЦИЯ 1: РЕЖИМ КАНАЛОВ (Моно / Стерео / M-S) ---
                        div { class: "flex flex-col gap-2",
                            div { class: "font-bold opacity-60 uppercase tracking-wider text-[10px]", "Channels mode" }

                            // Переключатель Mid / Side мониторинга (доступен если не включено моно)
                        div { class: "flex flex-col gap-1.5 mt-2 select-none",
                            span { class: "text-[11px] font-medium opacity-60 px-0.5", "Isolate into M/S:" }
                            div { class: "flex p-1 rounded-xl bg-surface-variant/40 border border-outline-variant/30 text-[11px] backdrop-blur-sm",
                                button {
                                    class: "flex-1 py-1.5 rounded-lg text-center transition-all duration-200 font-medium text-[11px] cursor-pointer",
                                    class: if ms_mode() == "none" { "bg-primary text-on-primary shadow-md transform scale-[1.02]" } else { "text-foreground/70 hover:text-primary hover:bg-surface-hover/30" },
                                    onclick: move |_| {
                                        ms_mode.set("none".to_string());
                                        let _ = ctx.command_tx.read().send(PlayerCommand::SetMidSideMode(0));
                                    },
                                    "Stereo"
                                }
                                button {
                                    class: "flex-1 py-1.5 rounded-lg text-center transition-all duration-200 font-medium text-[11px] cursor-pointer",
                                    class: if ms_mode() == "mid" { "bg-primary text-on-primary shadow-md transform scale-[1.02]" } else { "text-foreground/70 hover:text-primary hover:bg-surface-hover/30" },
                                    onclick:  move |_| {
                                        ms_mode.set("mid".to_string());
                                        let _ = ctx.command_tx.read().send(PlayerCommand::SetMidSideMode(1));
                                    },
                                    "Mid"
                                }
                                button {
                                    class: "flex-1 py-1.5 rounded-lg text-center transition-all duration-200 font-medium text-[11px] cursor-pointer",
                                    class: if ms_mode() == "side" { "bg-primary text-on-primary shadow-md transform scale-[1.02]" } else { "text-foreground/70 hover:text-primary hover:bg-surface-hover/30" },
                                    onclick: move |_| {
                                        ms_mode.set("side".to_string());
                                        let _ = ctx.command_tx.read().send(PlayerCommand::SetMidSideMode(2));
                                    },
                                    "Side"
                                }
                            }
                        }

                        div { class: "h-[1px] bg-outline-variant/30 w-full" }

                        // --- СЕКЦИЯ 2: СТЕРЕОБАЗА И ПАНОРАМА ---
                        div { class: "flex flex-col gap-3",
                            div { class: "font-bold opacity-60 uppercase tracking-wider text-[10px]", "Spatial" }

                            div { class: "flex flex-row justify-around gap-2 w-full pt-1",

                                // Ползунок расширения стереобазы (отключается в моно)
                                AudioKnob {
                                    label: "Stereo Width".to_string(),
                                    value: stereo_width,
                                    default_value: 1.0,
                                    min: 0.0, max: 2.0, step: 0.1,
                                    display_value: format!("{:.1}x", stereo_width()),
                                    disabled: false,
                                    on_value_change: {
                                        let ctx = ctx.clone();
                                        move |w| {
                                            let _ = ctx.command_tx.read().send(PlayerCommand::SetStereoWidth(w));
                                        }
                                    },
                                }

                                // Ползунок панорамирования (Баланс Лево / Право)
                                AudioKnob {
                                    label: "Pan (L/R)".to_string(),
                                    value: pan,
                                    default_value: 0.0,
                                    min: -1.0, max: 1.0, step: 0.05,
                                    display_value: pan_text.as_str(),
                                    disabled: false,
                                    on_value_change: {
                                        let ctx = ctx.clone();
                                        move |p| {
                                            let _ = ctx.command_tx.read().send(PlayerCommand::SetPan(p));
                                        }
                                    },
                                }
                        }

                        div { class: "h-[1px] bg-outline-variant/30 w-full" }


                        // --- СЕКЦИЯ 3: ФИЛЬТРЫ (HPF / LPF) ---
                        div { class: "flex flex-col gap-3",
                            div { class: "font-bold opacity-60 uppercase tracking-wider text-[10px]", "LP/HP Filters" }

                            // Выстраиваем две независимые колонки в один ряд
                                div { class: "flex flex-row justify-around gap-2 w-full pt-1",

                                    // Левая колонка: Low-Pass (Чекбокс + Ручка)
                                    div { class: "flex flex-col items-center",
                                        // Этот контейнер принудительно центрирует SettingsCheckbox по своей оси
                                        div { class: "",
                                            SettingsCheckbox {
                                                id: "enable-lp-filter",
                                                text: "".to_string(),
                                                checked: lp_enabled,
                                                onchange: move |_| {
                                                    let new_state = !lp_enabled();
                                                    lp_enabled.set(new_state);
                                                    let _ = ctx.command_tx.read().send(PlayerCommand::SetLowPassFilter(
                                                        if new_state { Some(lp_cutoff()) } else { None },
                                                    ));
                                                },
                                            }
                                        }
                                        // Сама ручка, которая встает строго под чекбоксом
                                        AudioKnob {
                                            label: "LP".to_string(),
                                            value: lp_cutoff,
                                            default_value: 20000.0,
                                            min: 5.0,
                                            max: 20000.0,
                                            step: 10.0,
                                            display_value: format!("{:.0} Hz", lp_cutoff),
                                            disabled: !lp_enabled(),
                                            logarithmic: true,
                                            on_value_change: {
                                                 // let ctx = ctx.clone();
                                                 // let lp_enabled = lp_enabled.clone();
                                                 move |c| {
                                                    if lp_enabled() {
                                                        let _ = ctx.command_tx.read().send(PlayerCommand::SetLowPassFilter(Some(c)));
                                                    }
                                                }
                                            },
                                        }
                                    }

                                    // Правая колонка: High-Pass (Чекбокс + Ручка)
                                    div { class: "flex flex-col items-center",
                                        // Этот контейнер принудительно центрирует SettingsCheckbox по своей оси
                                        div { class: "",
                                            SettingsCheckbox {
                                                id: "enable-hp-filter",
                                                text: "".to_string(),
                                                checked: hp_enabled,
                                                onchange: move |_| {
                                                    let new_state = !hp_enabled();
                                                    hp_enabled.set(new_state);
                                                    let _ = ctx.command_tx.read().send(PlayerCommand::SetHighPassFilter(
                                                        if new_state { Some(hp_cutoff()) } else { None },
                                                    ));
                                                },
                                            }
                                        }
                                        // Сама ручка, которая встает строго под чекбоксом
                                        AudioKnob {
                                            label: "HP".to_string(),
                                            value: hp_cutoff,
                                            default_value: 20.0,
                                            min: 20.0,
                                            max: 20500.0,
                                            step: 10.0,
                                            display_value: format!("{:.0} Hz", hp_cutoff),
                                            disabled: !hp_enabled(),
                                            logarithmic: true,
                                            on_value_change: {
                                                // let ctx = ctx.clone();
                                                // let hp_enabled = hp_enabled.clone();
                                                move |c| {
                                                    if hp_enabled() {
                                                        let _ = ctx.command_tx.read().send(PlayerCommand::SetHighPassFilter(Some(c)));
                                                    }
                                                }
                                            }
                                        }
                                    }

                                }

                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// Глобальный потокобезопасный счетчик для генерации уникальных ID ручек
static KNOB_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Вспомогательный компонент поворотной ручки (Audio Knob).
/// Реализует крутилку через SVG с обработкой перетаскивания мыши.
#[component]
fn AudioKnob(
    label: String,
    value: Signal<f32>,
    default_value: f32,
    min: f32,
    max: f32,
    step: f32,
    display_value: String,
    disabled: Option<bool>,
    #[props(optional)] logarithmic: Option<bool>,
    #[props(optional)] on_value_change: Option<EventHandler<f32>>,
) -> Element {
    let is_disabled = disabled.unwrap_or(false);
    let is_logarithmic = logarithmic.unwrap_or(false);
    let mut is_dragging = use_signal(|| false);
    let mut start_y = use_signal(|| 0.0f32);
    let mut start_val = use_signal(|| 0.0f32);

    // Уникальный ID для нашей ручки
    // use_hook гарантирует, что ID сгенерируется ОДИН раз при создании компонента
    // и сохранится между перерендерами
    let knob_id = use_signal(|| -> String {
        let id = KNOB_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("audio-knob-{}", id)
    });

    // Нормализованная позиция 0..1: линейная или логарифмическая.
    let percent = if is_logarithmic {
        ((value().ln() - min.ln()) / (max.ln() - min.ln())).clamp(0.0, 1.0)
    } else {
        ((value() - min) / (max - min)).clamp(0.0, 1.0)
    };
    let rotation = -135.0 + (percent * 270.0);

    let opacity_class = if is_disabled {
        "opacity-25 pointer-events-none"
    } else {
        ""
    };

    // colorize knobs circle bars
    let tick_count = 21;
    let active_ticks = ((percent * tick_count as f32).round() as usize).min(tick_count);

    let active_dasharray = if active_ticks == 0 {
        "0 1000".to_string()
    } else {
        format!(
            "{} 0 1000",
            std::iter::repeat_n("0.4 3.6", active_ticks)
                .collect::<Vec<_>>()
                .join(" ")
        )
    };

    // Эффект увеличения scale-[1.03] полностью УДАЛЕН.
    // Теперь геометрия элемента при клике абсолютно статична.
    let knob_style_classes = if is_dragging() {
        "border-outline-variant/60 bg-surface-variant/20 shadow-inner"
    } else {
        "border-outline-variant/60 bg-surface-variant/20 hover:shadow-md transition-shadow"
    };

    rsx! {
        div {
            class: "flex flex-col items-center gap-1.5 flex-1 min-w-[70px] select-none transition-opacity duration-150 {opacity_class}",

            span { class: "text-[10px] font-medium opacity-60 text-center tracking-tight truncate w-full", "{label}" },

            // Корпус крутилки
            div {
                // Присваиваем ID, чтобы найти элемент из JS
                id: knob_id(),
                class: "relative w-12 h-12 rounded-full border flex items-center justify-center cursor-ns-resize transition-all duration-150 shadow-sm focus:outline-none focus:ring-0 active:bg-transparent select-none {knob_style_classes}",
                style: "-webkit-tap-highlight-color: transparent; touch-action: none;",

                onpointerdown: move |e| {
                    is_dragging.set(true);
                    start_y.set(e.screen_coordinates().y as f32);
                    start_val.set(value());

                    let p_id = e.pointer_id();
                    let knob_id = knob_id();

                    // Кроссплатформенный вызов JS кода через eval
                    let js_code = format!(
                        "const el = document.getElementById('{knob_id}'); \
                         if (el) el.setPointerCapture({p_id});"
                    );
                    let _ = document::eval(&js_code);
                },

                onpointermove: move |e| {
                    if !is_dragging() {
                        return;
                    }
                    let delta_y = (start_y() - e.screen_coordinates().y as f32) as f32;
                    let next = if is_logarithmic {
                        // Лог-шкала: одинаковый угол ручки = одинаковый диапазон на слух.
                        let ln_min = min.ln();
                        let range = (max / min).ln();
                        let start_t = (start_val().ln() - ln_min) / range;
                        let t = (start_t + delta_y / 150.0).clamp(0.0, 1.0);
                        (ln_min + t * range).exp().clamp(min, max)
                    } else {
                        // Линейная шкала (например, для Pan).
                        let raw = start_val() + (delta_y / 150.0) * (max - min);
                        let stepped = (raw / step).round() * step;
                        stepped.clamp(min, max)
                    };

                    // Не дублируем и не шлём, если значение не изменилось
                    if next != value() {
                        value.set(next);
                        if let Some(cb) = &on_value_change {
                            cb.call(next);
                        }
                    }
                },
                onpointerup: move |evt| {
                    // if !is_dragging() { return; }
                    is_dragging.set(false);
                    let knob_id = knob_id();

                    let p_id = evt.pointer_id();
                    let js_code = format!(
                        "const el = document.getElementById('{knob_id}'); \
                         if (el) el.releasePointerCapture({p_id});"
                    );
                    let _ = document::eval(&js_code);
                },
                onpointerleave: move |_| {
                    is_dragging.set(false);
                },
                onpointercancel: move |evt| {
                    // if !is_dragging() { return; }
                    is_dragging.set(false);
                    let knob_id = knob_id();

                    let p_id = evt.pointer_id();
                    let js_code = format!(
                        "const el = document.getElementById('{knob_id}'); \
                         if (el) el.releasePointerCapture({p_id});"
                    );
                    let _ = document::eval(&js_code);
                },

                onclick: move |_| {
                    is_dragging.set(false);
                },

                // DONE
                ondoubleclick: move |_| {
                    is_dragging.set(false);
                    value.set(default_value);
                    if let Some(cb) = &on_value_change {
                        cb.call(default_value);
                    }
                },

                svg {
                    class: "w-11 h-11 relative overflow-visible pointer-events-none select-none",
                    view_box: "0 0 40 40",

                    // 1. Внешнее кольцо с делениями.
                    // Вместо класса цвета теперь используется явный инлайновый stroke: "#d0d0d0".
                    // Это гарантирует мягкий, благородный светло-серый оттенок делений.
                    // Серые оригинальные риски
                    path {
                        d: "M 7.6 32.4 A 17.5 17.5 0 1 1 32.4 32.4",
                        fill: "none",
                        stroke: "#d0d0d0",
                        stroke_width: "3.2",
                        stroke_dasharray: "0.4 3.6",
                    }

                    // Цветной бордер
                    path {
                        class: "text-primary",
                        d: "M 7.6 32.4 A 17.5 17.5 0 1 1 32.4 32.4",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_opacity: "0.25",
                        stroke_width: "4.4",
                        stroke_dasharray: active_dasharray.clone(),
                    }

                    // Цветные активные риски
                    path {
                        class: "text-primary",
                        d: "M 7.6 32.4 A 17.5 17.5 0 1 1 32.4 32.4",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_opacity: "0.55",
                        stroke_width: "3.2",
                        stroke_dasharray: active_dasharray.clone(),
                    }

                    // 2. Внутренний вращающийся диск
                    g {
                        class: "transition-transform duration-75 origin-[20px_20px] pointer-events-none select-none",
                        style: "transform: rotate({rotation}deg);",

                        circle {
                            cx: "20",
                            cy: "20",
                            r: "12.5",
                            fill: "#608080",
                            class: "shadow-sm"
                        }

                        circle {
                            cx: "20", cy: "20", r: "12.5",
                            fill: "none",
                            stroke: "rgba(255,255,255,0.2)",
                            stroke_width: "1.0"
                        }

                        line {
                            x1: "20", y1: "20", x2: "20", y2: "9",
                            stroke: "#e0e0e0",
                            stroke_width: "2.5",
                            stroke_linecap: "round"
                        }
                    }
                }
            }

            span { class: "font-mono font-bold text-primary text-[10px] bg-primary/10 px-2 py-0.5 rounded-md text-center min-w-[50px]", "{display_value}" }
        }
    }
}
