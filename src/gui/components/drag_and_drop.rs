use super::open_file::OpenFile;
use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;
use dioxus::prelude::*;

#[cfg(not(target_os = "android"))]
#[component]
pub fn DragAndDropFile(is_dragover_event: Signal<bool>) -> Element {
    let ctx = use_context::<PlayerContext>();

    let zone_class = if is_dragover_event() {
        "border-primary bg-primary/10"
    } else {
        "border-outline/60 hover:border-primary/60 hover:bg-surface-variant/20"
    };

    rsx! {
        if is_dragover_event() {
            div {
                class: "absolute inset-0 z-10 flex flex-col
                        items-center justify-center gap-2
                        select-none cursor-pointer
                        rounded-2xl border-2 border-dashed
                        pointer-events-auto
                        transition-all duration-200
                        {zone_class}",

                onclick: move |_| {
                    let ctx = ctx;

                    spawn(async move {
                        if let Some(file) = rfd::AsyncFileDialog::new()
                            .add_filter(
                                "Audio",
                                &["mp3", "wav", "flac", "ogg", "m4a", "aac"],
                            )
                            .set_title("Select audio file")
                            .pick_file()
                            .await
                        {
                            let path = file
                                .path()
                                .to_string_lossy()
                                .into_owned();

                            let _ = ctx
                                .command_tx
                                .read()
                                .send(PlayerCommand::LoadFile(path));
                        }
                    });
                },

                span {
                    class: "material-symbols-outlined text-primary/70",
                    style: "font-size: 36px;",
                    "audio_file"
                }

                span {
                    class: "hidden text-sm font-medium
                            text-primary/90
                            lg:block",

                    "Drag & Drop an audio file here or click to browse"
                }

                span {
                    class: "text-sm font-medium
                            text-primary/90
                            lg:hidden",

                    "Tap/click to browse your tracks"
                }

                span {
                    class: "text-[11px] text-secondary opacity-80",

                    "mp3, wav, flac, ogg, m4a, aac"
                }
            }
        }
    }
}

#[cfg(target_os = "android")]
#[component]
pub fn DragAndDropFile(is_dragover_event: Signal<bool>) -> Element {
    let _ = is_dragover_event;

    rsx! {
        div {
            class: "flex items-center justify-center rounded-2xl \
                    border-2 border-dashed border-outline/60 p-4",

            OpenFile {
                compact: false
            }
        }
    }
}
