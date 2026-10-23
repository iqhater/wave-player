use crate::gui::layout::PlaybackUIState;
use dioxus::prelude::*;

#[component]
pub fn BpmKey() -> Element {
    let playback = use_context::<PlaybackUIState>();

    let bpm = playback.bpm;
    let key = playback.key_name;
    let key_numerical = playback.key_numerical;

    let camelot = move || {
        let numerical = key_numerical();

        if numerical.is_empty() {
            String::new()
        } else {
            format!("({numerical})")
        }
    };

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
                span { class: "text-2xl font-bold text-primary lg:text-xl", "{key()} {camelot()}" }
            }
        }
    }
}
