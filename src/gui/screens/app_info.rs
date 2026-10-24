use crate::PlayerContext;
use crate::gui::layout::UpdateUIState;
use crate::updater::{UpdateCommand, UpdateState};
use dioxus::prelude::*;

#[component]
pub fn AppInfo() -> Element {
    let ctx = use_context::<PlayerContext>();
    let update = use_context::<UpdateUIState>();
    let update_state = update.state;

    rsx! {
        div {
            class: "w-full flex flex-col gap-6",

            h1 {
                class: "text-2xl font-semibold text-secondary",
                "App Info"
            }

            div {
                class: "w-full rounded-2xl bg-surface-container-lowest
                        p-6 shadow-[0_4px_20px_rgba(107,122,143,0.06)]",

                div {
                    class: "flex items-center gap-4",

                    div {
                        class: "flex h-16 w-16 shrink-0 items-center justify-center
                                rounded-2xl bg-primary text-on-primary",

                        span {
                            class: "material-symbols-outlined text-[36px]",
                            "info"
                        }
                    }

                    div {
                        class: "min-w-0",

                        h2 {
                            class: "text-xl font-semibold text-primary",
                            "Wave Player"
                        }

                        p {
                            class: "text-sm tracking-widest uppercase text-secondary",
                            "Audio Space"
                        }
                    }
                }

                div {
                    class: "mt-6 border-t border-surface-container pt-5",

                    div {
                        class: "flex items-center justify-between gap-4
                                border-b border-surface-container py-3",

                        span {
                            class: "text-sm font-medium text-secondary",
                            "Version"
                        }

                        span {
                            class: "text-sm font-semibold text-on-surface",
                            "{crate::shared::version_label()}"
                        }
                    }

                    if crate::updater::is_supported() {
                        div {
                            class: "flex items-center justify-between gap-4
                                    border-b border-surface-container py-3",

                            span {
                                class: "text-sm font-medium text-secondary",
                                "Updates"
                            }

                            button {
                                class: "cursor-pointer text-xs text-secondary
                                        transition-colors hover:text-primary
                                        disabled:cursor-not-allowed disabled:opacity-50",
                                disabled: matches!(
                                    update_state(),
                                    UpdateState::Checking | UpdateState::Downloading { .. }
                                ),
                                onclick: move |_| {
                                    let _ = ctx.update_cmd_tx.read().try_send(UpdateCommand::Check);
                                },
                                "Check for updates"
                            }
                        }
                    }

                    {match update_state() {
                        UpdateState::Checking => rsx! {
                            p { class: "mt-3 text-xs text-secondary", "Checking for updates..." }
                        },
                        UpdateState::UpToDate => rsx! {
                            p { class: "mt-3 text-xs text-secondary", "You're up to date." }
                        },
                        UpdateState::Available(info) => rsx! {
                            p { class: "mt-3 text-xs text-primary", "Version {info.version} is available." }
                        },
                        UpdateState::Downloading { downloaded, total, .. } => {
                            let percent = total
                                .filter(|total| *total > 0)
                                .map(|total| (downloaded as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
                                .unwrap_or(0.0);

                            rsx! {
                                p {
                                    class: "mt-3 text-xs text-secondary",
                                    "Downloading update... {percent:.0}%"
                                }
                            }
                        },
                        UpdateState::ReadyToRestart(_) => rsx! {
                            p {
                                class: "mt-3 text-xs text-primary",
                                "Update ready. Restart to apply."
                            }
                        },
                        UpdateState::Error { message, .. } => rsx! {
                            p {
                                class: "mt-3 text-xs text-error break-words",
                                "{message}"
                            }
                        },
                        UpdateState::Idle => rsx! {},
                    }}

                    {match update_state() {
                        UpdateState::Available(_) => rsx! {
                            button {
                                class: "mt-2 cursor-pointer text-xs font-medium text-primary hover:underline",
                                onclick: move |_| {
                                    let _ = ctx.update_cmd_tx.read().try_send(UpdateCommand::Download);
                                },
                                "Download update"
                            }
                        },
                        UpdateState::ReadyToRestart(_) => rsx! {
                            button {
                                class: "mt-2 cursor-pointer text-xs font-medium text-primary hover:underline",
                                onclick: move |_| {
                                    ctx.save_config_now();
                                    let _ = ctx
                                        .update_cmd_tx
                                        .read()
                                        .try_send(UpdateCommand::ApplyAndRestart);
                                },
                                "Restart to update"
                            }
                        },
                        UpdateState::Error { info: Some(_), .. } => rsx! {
                            button {
                                class: "mt-2 cursor-pointer text-xs font-medium text-primary hover:underline",
                                onclick: move |_| {
                                    let _ = ctx.update_cmd_tx.read().try_send(UpdateCommand::Download);
                                },
                                "Retry update"
                            }
                        },
                        _ => rsx! {},
                    }}

                    p {
                        class: "mt-5 text-sm leading-6 text-secondary",
                        "Wave Player is a desktop audio workspace for listening to
                         and exploring music."
                    }
                }
            }
        }
    }
}
