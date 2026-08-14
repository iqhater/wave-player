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
        div { class: "text-center",
            h2 { class: "font-headline-md text-headline-md text-on-surface mb-1",
                if title.is_empty() { "No track loaded" } else { "{title()}" }
            }
            p { class: "font-body-md text-body-md text-secondary opacity-80",
                if artist.is_empty() { "—" } else { "{artist()}" }
            }
        }
    }
}
