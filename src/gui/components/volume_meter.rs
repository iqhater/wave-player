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

#[component]
pub fn VolumeMeter() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut peak_db = use_signal(|| -60.0f32);
    let mut rms_db = use_signal(|| -60.0f32);

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();

        async move {
            while let Some(event) = status_rx.next().await {
                match event {
                    PlayerEvent::OutputLevelMeasured {
                        peak_db: peak,
                        rms_db: rms,
                    } => {
                        peak_db.set(normalize_db(peak));
                        rms_db.set(normalize_db(rms));
                    }

                    PlayerEvent::Started { .. } | PlayerEvent::Paused | PlayerEvent::Finished => {
                        peak_db.set(-60.0);
                        rms_db.set(-60.0);
                    }

                    _ => {}
                }
            }
        }
    });

    let peak = peak_db();
    let rms = rms_db();

    let peak_percent = db_to_level_percent(peak);
    let rms_percent = db_to_level_percent(rms);

    rsx! {
        div {
            class: "pointer-events-none hidden h-full w-14 shrink-0 select-none items-stretch gap-0 sm:flex",
            "aria-label": "Output level meter",

            div {
                class: "relative flex h-full w-7 shrink-0 items-center justify-center",

                span {
                    class: "absolute -top-4 left-1/2 -translate-x-1/2 whitespace-nowrap font-mono text-[9px] font-bold text-primary",
                    "{peak:.1} dB"
                }

                div {
                    class: "relative h-full w-2 overflow-hidden rounded-full bg-surface-variant/60 shadow-inner",

                    div {
                        class: "volume-meter-rms absolute inset-x-0 bottom-0 rounded-full",
                        style: "height: {rms_percent}%;",
                    }

                    div {
                        class: "volume-meter-peak absolute left-[-2px] right-[-2px] h-px rounded-full bg-on-surface",
                        style: "bottom: {peak_percent}%;",
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
                                    style: "top: {db_to_top_percent(db)}%; ; right: 10px;",

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
        value.clamp(-60.0, 0.0)
    } else {
        -60.0
    }
}

fn db_to_top_percent(value: f32) -> f32 {
    let db = normalize_db(value);

    // 0 dB = 0% сверху
    // -60 dB = 100% снизу
    ((-db) / 60.0 * 100.0).clamp(0.0, 100.0)
}

fn db_to_level_percent(value: f32) -> f32 {
    let db = normalize_db(value);

    // -60 dB = 0% заполнения
    // 0 dB = 100% заполнения
    ((db + 60.0) / 60.0 * 100.0).clamp(0.0, 100.0)
}
