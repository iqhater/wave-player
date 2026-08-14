use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;
use dioxus::prelude::*;

#[component]
pub fn OpenFile(compact: bool) -> Element {
    let ctx = use_context::<PlayerContext>();

    rsx! {
        // Open file button
        div {
           class: if compact {
               "px-0 mt-4 flex justify-center"
           } else {
               "px-4 mt-4"
           },

           button {
               class: if compact {
                   "w-12 h-12 rounded-xl text-primary font-bold \
                    hover:bg-surface-container-low cursor-pointer \
                    transition-all active:scale-95 \
                    flex items-center justify-center"
               } else {
                   "w-full py-2 px-3 rounded-xl text-primary font-bold \
                    hover:bg-surface-container-low cursor-pointer \
                    transition-all active:scale-95 \
                    flex items-center gap-2 justify-center"
               },

               title: if compact {
                   "Open file"
               } else {
                   ""
               },

               "aria-label": "Open file",

               onclick: move |_| {
                   spawn(async move {
                       if let Some(path) = rfd::AsyncFileDialog::new()
                           .add_filter(
                               "Audio",
                               &["mp3", "wav", "flac", "ogg", "m4a", "aac"],
                           )
                           .set_title("Select audio file")
                           .pick_file()
                           .await
                       {
                           let _ = ctx.command_tx.read().send(
                               PlayerCommand::LoadFile(
                                   path.path()
                                       .to_string_lossy()
                                       .to_string(),
                               ),
                           );
                       }
                   });
               },

               span {
                   class: "material-symbols-outlined text-[20px]",
                   "add"
               }

               if !compact {
                   span { "Open file" }
               }
           }
        }
    }
}
