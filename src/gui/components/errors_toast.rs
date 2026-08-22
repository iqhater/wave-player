use crate::AudioError;
use crate::PlayerContext;
use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn ErrorsToast() -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut current_error = use_signal(|| None::<AudioError>);

    // Listen errors events from audio engine
    use_future(move || {
        let mut error_rx = ctx.error_rx.read().clone();
        async move {
            while let Some(error) = error_rx.next().await {
                current_error.set(Some(error));
            }
        }
    });

    rsx! {
        if let Some(error) = current_error() {
            div {
                class: "fixed top-5 right-5 z-50 w-96 max-w-[calc(100vw-2rem)]",

                div {
                    class: "flex items-start gap-3 rounded-2xl border border-error/30 bg-error-container p-4 text-on-error-container shadow-xl",

                    span {
                        class: "material-symbols-outlined mt-0.5 shrink-0 text-[24px]",
                        style: "font-variation-settings: 'FILL' 1;",
                        "warning"
                    }

                    div {
                        class: "min-w-0 flex-1",

                        div {
                            class: "font-bold text-sm",
                            "Playback error"
                        }

                        div {
                            class: "mt-1 text-sm leading-5 opacity-90 break-words",
                            "{format_error_message(&error)}"
                        }
                    }

                    button {
                        class: "flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-on-error-container/70 transition-colors hover:bg-error/15 hover:text-on-error-container cursor-pointer",
                        title: "Close notification",
                        onclick: move |_| current_error.set(None),

                        span {
                            class: "material-symbols-outlined text-[20px]",
                            "close"
                        }
                    }
                }
            }
        }
    }
}

fn format_error_message(error: &AudioError) -> String {
    match error {
        AudioError::FileNotFound(path) => {
            format!("File not found: {path}")
        }
        AudioError::PermissionDenied => "No permission to access the audio file.".to_string(),
        AudioError::UnsupportedFormat => "This audio format is not supported.".to_string(),
        AudioError::SeekFailed => "Seeking failed for this audio source.".to_string(),
        AudioError::FileCorrupted => {
            "The audio file is corrupted or has an invalid structure.".to_string()
        }
        AudioError::DeviceInitializationFailed => {
            "The audio device could not be initialized.".to_string()
        }
        AudioError::DeviceLost => "The audio device was disconnected.".to_string(),
        AudioError::RenderAudioFailed => "Audio export failed.".to_string(),
    }
}
