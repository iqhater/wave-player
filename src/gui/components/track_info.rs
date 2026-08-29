use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn TrackInfo() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut title = use_signal(|| "".to_string());
    let mut artist = use_signal(|| "".to_string());

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started {
                        duration: _,
                        metadata,
                    } => {
                        artist.set(metadata.artist);
                        title.set(metadata.title);
                        println!("DEBUG EVENT from title: PlayerEvent::Started");
                    }
                    _ => {} // ignore other variants
                }
            }
        }
    });

    rsx! {
        div { class: "w-full min-w-0 px-2 text-center",
            h2 { class: "mb-1 w-full truncate
                        text-xl font-semibold text-on-surface
                        lg:text-headline-md",
                if title.is_empty() { "No track loaded" } else { "{title()}" }
            }
            p { class: "w-full truncate text-base text-secondary opacity-80
                        lg:text-body-md",
                if artist.is_empty() { "—" } else { "{artist()}" }
            }
        }
    }
}
