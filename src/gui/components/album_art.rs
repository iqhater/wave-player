use crate::gui::layout::PlaybackUIState;

use dioxus::prelude::*;
use std::fs;
use std::path::PathBuf;

/// Helper: bytes → base64 data URL for JPEG
fn jpeg_data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:image/jpeg;base64,{}", b64)
}

#[component]
pub fn AlbumArt() -> Element {
    let playback = use_context::<PlaybackUIState>();

    let img_src = playback
        .metadata
        .read()
        .as_ref()
        .filter(|metadata| !metadata.cover.is_empty())
        .map(|metadata| jpeg_data_url(&metadata.cover))
        .unwrap_or_default();

    let has_cover = !img_src.is_empty();

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

#[component]
pub fn TrackThumbnail(cache_path: Option<PathBuf>) -> Element {
    let cover_bytes = cache_path
        .as_deref()
        .and_then(|path| fs::read(path).ok())
        .filter(|bytes| !bytes.is_empty());

    let image_source = cover_bytes.as_deref().map(jpeg_data_url);

    rsx! {
        div {
            class: "w-10 h-10 shrink-0 rounded-lg overflow-hidden \
                    bg-surface-container flex items-center justify-center",

            if let Some(image_source) = image_source {
                img {
                    src: "{image_source}",
                    alt: "Album cover",
                    class: "w-full h-full object-cover",
                }
            } else {
                span {
                    class: "material-symbols-outlined text-[20px] \
                            text-secondary",
                    "music_note"
                }
            }
        }
    }
}
