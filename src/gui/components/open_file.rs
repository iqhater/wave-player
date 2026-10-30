use crate::PlayerContext;
use crate::shared::command_types::PlayerCommand;

#[cfg(not(target_os = "android"))]
use crate::shared::SUPPORTED_AUDIO_EXTENSIONS;

#[cfg(target_os = "android")]
use dioxus::html::{FileData, HasFileData};

#[cfg(target_os = "android")]
use std::hash::{Hash, Hasher};

use dioxus::prelude::*;

#[cfg(target_os = "android")]
use std::path::PathBuf;

#[cfg(not(target_os = "android"))]
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
                           .add_filter("Audio", SUPPORTED_AUDIO_EXTENSIONS)
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

#[cfg(target_os = "android")]
#[component]
pub fn OpenFile(compact: bool) -> Element {
    let ctx = use_context::<PlayerContext>();

    rsx! {
        label {
            class: if compact {
                "relative w-12 h-12 rounded-xl text-primary font-bold \
                 flex items-center justify-center cursor-pointer \
                 hover:bg-surface-container-low transition-all active:scale-95"
            } else {
                "relative w-full py-2 px-3 rounded-xl text-primary font-bold \
                 flex items-center gap-2 justify-center cursor-pointer \
                 hover:bg-surface-container-low transition-all active:scale-95"
            },

            title: "Open file",
            "aria-label": "Open file",

            span {
                class: "material-symbols-outlined text-[20px]",
                "add"
            }

            if !compact {
                span { "Open file" }
            }

            input {
                id: "android-audio-file-picker",
                r#type: "file",
                accept: "audio/*,.mp3,.wav,.flac,.ogg,.m4a,.aac",
                class: "absolute inset-0 z-10 h-full w-full cursor-pointer opacity-0",

                onchange: move |event| {
                    let Some(file) = event.files().into_iter().next() else {
                        return;
                    };

                    let ctx = ctx;

                    spawn(async move {
                        match stage_android_file(file).await {
                            Ok(path) => {
                                let _ = ctx.command_tx.read().send(
                                    PlayerCommand::LoadFile(
                                        path.to_string_lossy().into_owned(),
                                    ),
                                );
                            }
                            Err(error) => {
                                eprintln!("{error}");
                            }
                        }
                    });
                }
            }
        }
    }
}

#[cfg(target_os = "android")]
async fn stage_android_file(file: FileData) -> Result<PathBuf, String> {
    let bytes = file
        .read_bytes()
        .await
        .map_err(|error| format!("Failed to read selected file: {error}"))?;

    let original_name = file.name();

    let safe_name: String = original_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();

    let safe_name = if safe_name.is_empty() {
        "audio_file".to_string()
    } else {
        safe_name
    };

    // Используем содержимое и метаданные файла для стабильного имени.
    // Это предотвращает конфликт файлов с одинаковыми именами.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    original_name.hash(&mut hasher);
    file.size().hash(&mut hasher);
    file.last_modified().hash(&mut hasher);
    bytes.as_ref().hash(&mut hasher);

    let file_name = format!("{:016x}_{safe_name}", hasher.finish());

    let mut directories = vec![std::env::temp_dir().join("wave_player_imports")];

    if let Ok(current_dir) = std::env::current_dir() {
        directories.push(current_dir.join("wave_player_data").join("imports"));
    }

    let mut last_error = None;

    for directory in directories {
        if let Err(error) = std::fs::create_dir_all(&directory) {
            last_error = Some(format!(
                "Failed to create import directory {}: {error}",
                directory.display()
            ));
            continue;
        }

        let path = directory.join(&file_name);

        match std::fs::write(&path, &bytes) {
            Ok(()) => return Ok(path),
            Err(error) => {
                last_error = Some(format!(
                    "Failed to save selected file {}: {error}",
                    path.display()
                ));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        "Failed to create a writable directory for the selected file".to_string()
    }))
}
