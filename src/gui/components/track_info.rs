use crate::gui::layout::PlaybackUIState;
use dioxus::prelude::*;

#[component]
pub fn TrackInfo() -> Element {
    let playback = use_context::<PlaybackUIState>();

    let (title, artist) = playback
        .metadata
        .read()
        .as_ref()
        .map(|metadata| (metadata.title.clone(), metadata.artist.clone()))
        .unwrap_or_default();

    rsx! {
        div { class: "w-full min-w-0 px-2 text-center",
            h2 { class: "mb-1 w-full truncate
                        text-xl font-semibold text-on-surface
                        lg:text-headline-md",
                if title.is_empty() { "No track loaded" } else { "{title}" }
            }
            p { class: "w-full truncate text-base text-secondary opacity-80
                        lg:text-body-md",
                if artist.is_empty() { "—" } else { "{artist}" }
            }
        }
    }
}
