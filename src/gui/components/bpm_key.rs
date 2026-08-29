use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn BpmKey() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut bpm = use_signal(|| 0.0f32);
    let mut key = use_signal(|| "—".to_string());
    let mut key_camelot = use_signal(|| "".to_string());

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::BpmKeyDetected {
                        bpm: b,
                        key_name,
                        key_numerical,
                        ..
                    } => {
                        bpm.set(b);
                        key.set(key_name);
                        key_camelot.set(format!("{}{}{}", "(", key_numerical, ")"));
                    }
                    PlayerEvent::Started { .. } => {
                        bpm.set(0.0);
                        key.set("—".to_string());
                        key_camelot.set("".to_string());
                    }
                    _ => {}
                }
            }
        }
    });

    rsx! {
        div { class: "flex items-start justify-center gap-12 lg:gap-4",
            div { class: "flex flex-col",
                span { class: "text-label-sm uppercase text-secondary opacity-60", "BPM" }
                span { class: "text-2xl font-bold text-primary lg:text-xl",
                    if bpm() > 0.0 { "{bpm()}" } else { "—" }
                }
            }
            div { class: "flex flex-col",
                span { class: "text-label-sm uppercase text-secondary opacity-60", "Key" }
                span { class: "text-2xl font-bold text-primary lg:text-xl", "{key()} {key_camelot()}" }
            }
        }
    }
}
