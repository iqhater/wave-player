use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;

/// Helper: bytes → base64 data URL for JPEG
fn jpeg_data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:image/jpeg;base64,{}", b64)
}

#[component]
pub fn AlbumArt() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut cover_bytes = use_signal(|| Vec::<u8>::new());

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    PlayerEvent::Started { metadata, .. } => {
                        cover_bytes.set(metadata.cover);
                    }
                    _ => {}
                }
            }
        }
    });

    let has_cover = cover_bytes.read().len() > 0;
    let img_src = if has_cover {
        jpeg_data_url(&cover_bytes.read())
    } else {
        String::new()
    };

    rsx! {
        div { class: "relative group w-full max-w-[300px]",
            div { class: "flex aspect-square w-full items-center justify-center
                            rounded-[32px] p-4
                            neumorph-raised
                            transition-transform duration-500
                            hover:scale-[1.02]",
                // style: "width: 300px; height: 300px;",
                div { class: "w-full h-full rounded-[24px] overflow-hidden shadow-sm flex items-center justify-center",
                    if has_cover {
                        img {
                            src: "{img_src}",
                            alt: "Album Art",
                            class: "w-full h-full object-cover",
                        }
                    } else {
                        // Placeholder icon when no cover
                        div { class: "absolute top-1/2 left-[50%] -translate-y-1/2 -translate-x-1/2 cursor-pointer loop-marker hover:scale-105 transition-transform",
                            span {
                                class: "material-symbols-outlined text-primary/20 text-[140px]",
                                style: "font-size: 140px; font-variation-settings: \"FILL\" 0;",
                                "music_note",
                            }
                        }
                    }
                }
            }
        }
    }
}
