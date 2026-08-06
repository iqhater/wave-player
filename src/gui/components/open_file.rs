use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;
use dioxus::prelude::*;

#[component]
pub fn OpenFile() -> Element {
    let ctx = use_context::<PlayerContext>();

    rsx! {
        // Open file button
        div { class: "px-4 mt-4",
            button {
                class: "w-full py-2 px-3 rounded-xl text-primary font-bold \
                        hover:bg-surface-container-higher hover:bg-surface-container-low cursor-pointer transition-all active:scale-95 \
                        flex items-center gap-2 justify-center",
                onclick: move |_| {
                    spawn(async move {
                        if let Some(path) = rfd::AsyncFileDialog::new()
                            .add_filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a", "aac"])
                            .set_title("Select audio file")
                            .pick_file()
                            .await
                        {
                            let _ = ctx.command_tx.read().send(PlayerCommand::LoadFile(
                                path.path().to_string_lossy().to_string(),
                            ));
                        }
                    });
                },
                span { class: "material-symbols-outlined text-[20px]", "add" }
                span { "Open file" }
            }
        }
    }
}
