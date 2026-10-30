use super::open_file::OpenFile;
use crate::PlayerContext;
use crate::gui::layout::SidebarCollapsed;
use crate::gui::layout::{MobileMenuOpen, UpdateUIState};
use crate::gui::route::Route;
use crate::updater::{UpdateCommand, UpdateInfo, UpdateState};
use dioxus::prelude::*;

fn update_info(state: &UpdateState) -> Option<&UpdateInfo> {
    match state {
        UpdateState::Available(info) | UpdateState::ReadyToRestart(info) => Some(info),
        UpdateState::Downloading { info, .. } => Some(info),
        UpdateState::Error {
            info: Some(info), ..
        } => Some(info),
        _ => None,
    }
}

#[component]
pub fn Sidebar() -> Element {
    let current_route = use_route::<Route>();
    let mut collapsed = use_context::<SidebarCollapsed>().0;
    let ctx = use_context::<PlayerContext>();
    let update = use_context::<UpdateUIState>();
    let update_state = update.state;
    let mut dismissed_update_tag = update.dismissed_tag;
    let mut mobile_menu_open = use_context::<MobileMenuOpen>().0;
    let compact = collapsed() && !mobile_menu_open();

    let current_update = update_state();
    let notice_info = update_info(&current_update).cloned();
    let show_update = notice_info
        .as_ref()
        .is_some_and(|info| dismissed_update_tag().as_deref() != Some(info.tag.as_str()));
    let dismiss_tag = notice_info.as_ref().map(|info| info.tag.clone());

    let aside_class = if compact {
        "fixed inset-0 z-50
         flex h-screen w-full flex-shrink-0 flex-col
         overflow-hidden bg-surface-container-high px-3 py-stack-lg
         shadow-[4px_4px_12px_rgba(0,0,0,0.05)]
         transition-[width,padding,transform] duration-300 ease-in-out
         lg:static lg:inset-auto lg:z-20 lg:h-screen lg:w-20 lg:flex"
    } else {
        "fixed inset-0 z-50
         flex h-screen w-full flex-shrink-0 flex-col
         overflow-hidden bg-surface-container-high px-gutter py-stack-lg
         shadow-[4px_4px_12px_rgba(0,0,0,0.05)]
         transition-[width,padding,transform] duration-300 ease-in-out
         lg:static lg:inset-auto lg:z-20 lg:h-screen lg:w-64 lg:flex"
    };

    rsx! {
        aside {
            class: if mobile_menu_open() {
                "{aside_class} translate-x-0"
            } else {
                "{aside_class} -translate-x-full lg:translate-x-0"
            },

            // Header
            div {
                class: "h-20 flex-shrink-0 mb-stack-md flex flex-col justify-between",

                div {
                    class: if compact {
                        "flex items-center justify-center"
                    } else {
                        "flex items-start justify-between gap-3"
                    },

                    if compact {
                        /* span {
                            class: "material-symbols-outlined text-primary text-[30px]",
                            title: "Wave Player",
                            "graphic_eq"
                        } */
                    } else {
                        div {
                            class: "min-w-0",

                            h1 {
                                class: "font-headline-lg text-headline-lg text-primary \
                                        tracking-tight font-semibold whitespace-nowrap",
                                "Wave Player"
                            }

                            p {
                                class: "font-label-sm text-label-sm text-secondary \
                                        tracking-widest uppercase whitespace-nowrap",
                                "Audio Space"
                            }
                        }
                    }

                    button {
                        class: if compact {
                            "flex-shrink-0 w-10 h-10 flex items-center justify-center \
                             rounded-xl text-secondary hover:text-primary \
                             hover:bg-surface-container-low cursor-pointer \
                             transition-all active:scale-95"
                        } else {
                            "flex-shrink-0 w-9 h-9 flex items-center justify-center \
                             rounded-xl text-secondary hover:text-primary \
                             hover:bg-surface-container-low cursor-pointer \
                             transition-all active:scale-95"
                        },

                        title: if mobile_menu_open() {
                            "Close menu"
                        } else if collapsed() {
                            "Expand sidebar"
                        } else {
                            "Collapse sidebar"
                        },

                        "aria-label": if mobile_menu_open() {
                            "Close menu"
                        } else if collapsed() {
                            "Expand sidebar"
                        } else {
                            "Collapse sidebar"
                        },

                        onclick: move |_| {
                            if mobile_menu_open() {
                                mobile_menu_open.set(false);
                            } else {
                                let next = !collapsed();
                                collapsed.set(next);
                                ctx.update_config(|config| {
                                    config.ui.sidebar_collapsed = next;
                                });
                            }
                        },

                        span {
                            class: "material-symbols-outlined text-[22px]",
                            if mobile_menu_open() {
                                "close"
                            } else if collapsed() {
                                "menu"
                            } else {
                                "menu_open"
                            }
                        }
                    }
                }

                // Divider
                div {
                    class: "w-full border-t border-outline-variant",
                }
            }

            // Menu Items
            nav {
                class: if compact {
                    "flex flex-col gap-unit items-center"
                } else {
                    "flex flex-col gap-unit"
                },

                SidebarButton {
                    screen: Route::Home,
                    icon: "play_circle",
                    label: "Player",
                    active: current_route == Route::Home,
                    compact: compact,
                }

                SidebarButton {
                    screen: Route::RecentTracks,
                    icon: "queue_music",
                    label: "Recent Tracks",
                    active: current_route == Route::RecentTracks,
                    compact: compact,
                }

                SidebarButton {
                    screen: Route::Favorites,
                    icon: "favorite",
                    label: "Favorites",
                    active: current_route == Route::Favorites,
                    compact: compact,
                }

                SidebarButton {
                    screen: Route::Settings,
                    icon: "settings",
                    label: "Settings",
                    active: current_route == Route::Settings,
                    compact: compact,
                }
            }

            // Dashed separator
            hr {
                class: if compact {
                    "w-full mx-auto mt-4 border-t border-dashed border-outline-variant"
                } else {
                    "w-1/2 mx-auto mt-4 border-t border-dashed border-outline-variant"
                }
            }

            // Open file button
            OpenFile {
                compact: compact,
            }

            // Bottom update/app info block
            if !compact && show_update {
                div {
                    class: "mt-auto w-[calc(100%-2rem)] max-w-[520px] mx-auto p-4
                            bg-primary rounded-2xl text-surface
                            border border-outline/10 shadow-lg
                            lg:w-full lg:max-w-none lg:mx-0",

                    div {
                        class: "flex items-start justify-between gap-2 mb-1",

                        p {
                            class: "text-[10px] text-on-surface font-medium
                                    tracking-wide font-label-md text-label-md font-bold truncate",
                            if let Some(info) = notice_info.as_ref() {
                                "Version {info.version}"
                            }
                        }

                        button {
                            class: "flex h-6 w-6 shrink-0 items-center justify-center
                                    rounded-full text-on-surface/70
                                    hover:bg-surface-bright/20 hover:text-on-surface
                                    cursor-pointer transition-colors",

                            title: "Close update notification",
                            "aria-label": "Close update notification",

                            onclick: move |_| {
                                if let Some(tag) = dismiss_tag.clone() {
                                    dismissed_update_tag.set(Some(tag));
                                }
                            },

                            span {
                                class: "material-symbols-outlined text-[18px]",
                                "close"
                            }
                        }
                    }

                    {match current_update {
                        UpdateState::Available(_) => rsx! {
                            p {
                                class: "text-sm font-semibold mb-3 leading-tight
                                        tracking-tight text-surface-bright",
                                "New update available"
                            }

                            button {
                                class: "w-full py-2 bg-surface-bright
                                        hover:bg-surface-container-low cursor-pointer
                                        text-on-surface font-semibold rounded-xl text-xs
                                        active:scale-[0.98] transition-all flex
                                        items-center justify-center gap-2
                                        border border-outline-variant/20 shadow-sm",
                                onclick: move |_| {
                                    let _ = ctx.update_cmd_tx.read().try_send(UpdateCommand::Download);
                                },

                                span {
                                    class: "material-symbols-outlined text-sm
                                            text-on-surface/90 font-normal",
                                    "update"
                                }

                                "Update Now"
                            }
                        },
                        UpdateState::Downloading { downloaded, total, .. } => {
                            let percent = total
                                .filter(|total| *total > 0)
                                .map(|total| (downloaded as f64 / total as f64 * 100.0).clamp(0.0, 100.0))
                                .unwrap_or(0.0);

                            rsx! {
                                div {
                                    class: "flex items-center justify-between gap-2 mb-2",
                                    p {
                                        class: "text-sm font-semibold leading-tight
                                                tracking-tight text-surface-bright",
                                        "Downloading update"
                                    }
                                    span {
                                        class: "text-[11px] font-semibold tabular-nums",
                                        "{percent:.0}%"
                                    }
                                }

                                div {
                                    class: "h-1.5 w-full overflow-hidden rounded-full bg-surface-bright/30",
                                    div {
                                        class: "h-full rounded-full bg-surface-bright transition-[width] duration-150",
                                        style: "width: {percent:.0}%;",
                                    }
                                }
                            }
                        },
                        UpdateState::ReadyToRestart(_) => rsx! {
                            p {
                                class: "text-sm font-semibold mb-1 leading-tight
                                        tracking-tight text-surface-bright",
                                "Update ready"
                            }
                            p {
                                class: "text-[11px] leading-4 text-surface-bright/80 mb-3",
                                "Restart to apply the update."
                            }

                            button {
                                class: "w-full py-2 bg-surface-bright
                                        hover:bg-surface-container-low cursor-pointer
                                        text-on-surface font-semibold rounded-xl text-xs
                                        active:scale-[0.98] transition-all flex
                                        items-center justify-center gap-2
                                        border border-outline-variant/20 shadow-sm",
                                onclick: move |_| {
                                    ctx.save_config_now();
                                    let _ = ctx.update_cmd_tx.read().try_send(UpdateCommand::ApplyAndRestart);
                                },

                                span {
                                    class: "material-symbols-outlined text-sm
                                            text-on-surface/90 font-normal",
                                    "restart_alt"
                                }

                                "Restart to update"
                            }
                        },
                        UpdateState::Error { message, .. } => rsx! {
                            p {
                                class: "text-sm font-semibold mb-1 leading-tight
                                        tracking-tight text-surface-bright",
                                "Update failed"
                            }
                            p {
                                class: "text-[11px] leading-4 text-surface-bright/80 mb-3
                                        max-h-20 overflow-y-auto break-words",
                                "{message}"
                            }

                            button {
                                class: "w-full py-2 bg-surface-bright
                                        hover:bg-surface-container-low cursor-pointer
                                        text-on-surface font-semibold rounded-xl text-xs
                                        active:scale-[0.98] transition-all flex
                                        items-center justify-center gap-2
                                        border border-outline-variant/20 shadow-sm",
                                onclick: move |_| {
                                    let _ = ctx.update_cmd_tx.read().try_send(UpdateCommand::Download);
                                },

                                span {
                                    class: "material-symbols-outlined text-sm
                                            text-on-surface/90 font-normal",
                                    "refresh"
                                }

                                "Retry"
                            }
                        },
                        _ => rsx! {},
                    }}
                }
            } else {
                div {
                    class: if compact {
                        "mt-auto flex w-full justify-center"
                    } else {
                        "mt-auto w-full"
                    },

                    SidebarButton {
                        screen: Route::AppInfo,
                        icon: "info",
                        label: "App Info",
                        active: current_route == Route::AppInfo,
                        compact: compact,
                    }
                }
            }
        }
    }
}

#[component]
fn SidebarButton(
    screen: Route,
    icon: String,
    label: String,
    active: bool,
    compact: bool,
) -> Element {
    let active_class = if active {
        "flex items-center py-3 rounded-xl text-primary font-bold \
          border-r-4 border-primary cursor-pointer \
          bg-surface-container-low transition-all"
    } else {
        "flex items-center py-3 rounded-xl text-secondary font-medium \
          hover:bg-surface-container-low cursor-pointer \
          transition-colors active:scale-95 duration-150"
    };

    let layout_class = if compact {
        "w-12 justify-center px-2"
    } else {
        "w-full gap-4 px-4"
    };

    let mut mobile_menu_open = use_context::<MobileMenuOpen>().0;

    rsx! {
        Link {
            class: if compact {
                "w-12"
            } else {
                "w-full"
            },
            to: screen,

            onclick: move |_| mobile_menu_open.set(false),

            button {
                class: "{active_class} {layout_class}",
                title: if compact { label.clone() } else { String::new() },
                "aria-label": label.clone(),

                span {
                    class: "material-symbols-outlined flex-shrink-0",
                    "{icon}"
                }

                if !compact {
                    span {
                        class: "font-body-md text-body-md whitespace-nowrap",
                        "{label}"
                    }
                }
            }
        }
    }
}
