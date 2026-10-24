use std::time::{Duration, Instant};

use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;

const LEVEL_MARKS: [(f32, &str); 11] = [
    (0.0, "0"),
    (-6.0, "-6"),
    (-12.0, "-12"),
    (-18.0, "-18"),
    (-24.0, "-24"),
    (-30.0, "-30"),
    (-36.0, "-36"),
    (-42.0, "-42"),
    (-48.0, "-48"),
    (-54.0, "-54"),
    (-60.0, "-60"),
];

const MIN_DB: f32 = -60.0;

/// Сколько линия peak hold стоит на максимуме.
const PEAK_HOLD: Duration = Duration::from_millis(1500);

/// Скорость спада peak hold после удержания.
const PEAK_DECAY_DB_PER_SEC: f32 = 24.0;

/// Peak hold подсвечивается красным только на 0 dBFS.
const CLIP_DB: f32 = 0.0;

#[component]
pub fn VolumeMeter() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut rms_db = use_signal(|| MIN_DB);

    let mut peak_hold_db = use_signal(|| MIN_DB);
    let mut peak_hold_at = use_signal(Instant::now);

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();

        async move {
            while let Some(event) = status_rx.next().await {
                match event {
                    PlayerEvent::OutputLevelMeasured {
                        peak_db: peak,
                        rms_db: rms,
                    } => {
                        let now = Instant::now();

                        let current_peak = normalize_db(peak);
                        let current_rms = normalize_db(rms);

                        rms_db.set(current_rms);

                        let held = peak_hold_db();
                        let held_at = peak_hold_at();

                        if current_peak >= held {
                            peak_hold_db.set(current_peak);
                            peak_hold_at.set(now);
                        } else {
                            let elapsed = now.saturating_duration_since(held_at);

                            if elapsed > PEAK_HOLD {
                                let decayed = held
                                    - PEAK_DECAY_DB_PER_SEC * (elapsed - PEAK_HOLD).as_secs_f32();

                                peak_hold_db.set(decayed.max(current_peak).clamp(MIN_DB, 0.0));
                            }
                        }
                    }

                    PlayerEvent::Started { .. } | PlayerEvent::Paused | PlayerEvent::Finished => {
                        rms_db.set(MIN_DB);

                        peak_hold_db.set(MIN_DB);
                        peak_hold_at.set(Instant::now());
                    }

                    _ => {}
                }
            }
        }
    });

    let rms = rms_db();
    let peak_hold = peak_hold_db();

    let rms_percent = db_to_level_percent(rms);
    let peak_percent = db_to_level_percent(peak_hold);

    // Красным выделяем именно отображаемый текущий уровень.
    let is_clipping = rms >= CLIP_DB;

    // Отдельно — клиппинг пика для маркера peak hold.
    let peak_is_clipping = peak_hold >= CLIP_DB;

    rsx! {
        div {
            class: "pointer-events-none hidden h-full w-14 shrink-0 select-none items-stretch gap-0 sm:flex",
            "aria-label": "Output level meter",
            "aria-valuemin": "-60",
            "aria-valuemax": "0",
            "aria-valuenow": "{rms:.1}",

            div {
                class: "relative flex h-full w-7 shrink-0 items-center justify-center",

                span {
                    class: if is_clipping {
                        "absolute -top-4 left-1/2 -translate-x-1/2 whitespace-nowrap font-mono text-[9px] font-bold text-error"
                    } else {
                        "absolute -top-4 left-1/2 -translate-x-1/2 whitespace-nowrap font-mono text-[9px] font-bold text-primary"
                    },

                    "{rms:.1} dB"
                }

                div {
                    class: "relative h-full w-2 overflow-hidden rounded-full bg-surface-variant/60 shadow-inner",

                    // Полноразмерный градиент с фиксированными зонами.
                    // clip-path открывает только часть до текущего RMS.
                    div {
                        class: "volume-meter-rms absolute inset-0 rounded-full",
                        style: "clip-path: inset({100.0 - rms_percent}% 0 0 0);",
                    }

                    // Peak hold: держится на максимуме и затем плавно спадает.
                    if peak_hold > MIN_DB + 0.1 {
                        div {
                            class: if peak_is_clipping {
                                "volume-meter-peak absolute left-[-2px] right-[-2px] h-[2px] rounded-full bg-error shadow-sm"
                            } else {
                                "volume-meter-peak absolute left-[-2px] right-[-2px] h-[2px] rounded-full bg-on-surface shadow-sm"
                            },
                            style: "bottom: {peak_percent}%;",
                        }
                    }
                }
            }

            div {
                class: "relative h-full w-7 text-right text-[8px] text-secondary/70",

                {
                    LEVEL_MARKS
                        .iter()
                        .enumerate()
                        .map(|(index, &(db, label))| {
                            rsx! {
                                div {
                                    key: "level-mark-{index}",
                                    class: "absolute right-0 flex -translate-y-1/2 items-center gap-1 whitespace-nowrap font-mono",
                                    style: "top: {db_to_top_percent(db)}%; right: 10px;",

                                    span {
                                        "{label}"
                                    }
                                }
                            }
                        })
                }
            }
        }
    }
}

fn normalize_db(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_DB, 0.0)
    } else {
        MIN_DB
    }
}

fn db_to_top_percent(value: f32) -> f32 {
    let db = normalize_db(value);

    // 0 dB = 0% сверху, -60 dB = 100% снизу.
    ((-db) / 60.0 * 100.0).clamp(0.0, 100.0)
}

fn db_to_level_percent(value: f32) -> f32 {
    let db = normalize_db(value);

    // -60 dB = 0% заполнения, 0 dB = 100% заполнения.
    ((db + 60.0) / 60.0 * 100.0).clamp(0.0, 100.0)
}
