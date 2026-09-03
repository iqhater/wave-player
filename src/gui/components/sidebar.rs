use super::open_file::OpenFile;
use crate::gui::layout::MobileMenuOpen;
use crate::gui::route::Route;
use dioxus::prelude::*;

#[component]
pub fn Sidebar() -> Element {
    let current_route = use_route::<Route>();
    let mut collapsed = use_signal(|| false);
    let mut mobile_menu_open = use_context::<MobileMenuOpen>().0;

    let aside_class = if collapsed() {
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
                    class: if collapsed() {
                        "flex items-center justify-center"
                    } else {
                        "flex items-start justify-between gap-3"
                    },

                    if collapsed() {
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
                        class: if collapsed() {
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
                            // collapsed.toggle();
                            if mobile_menu_open() {
                                mobile_menu_open.set(false);
                            } else {
                                collapsed.toggle();
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
                class: if collapsed() {
                    "flex flex-col gap-unit items-center"
                } else {
                    "flex flex-col gap-unit"
                },

                SidebarButton {
                    screen: Route::Home,
                    icon: "play_circle",
                    label: "Player",
                    active: current_route == Route::Home,
                    compact: collapsed(),
                }

                SidebarButton {
                    screen: Route::RecentTracks,
                    icon: "queue_music",
                    label: "Recent Tracks",
                    active: current_route == Route::RecentTracks,
                    compact: collapsed(),
                }

                SidebarButton {
                    screen: Route::Favorites,
                    icon: "favorite",
                    label: "Favorites",
                    active: current_route == Route::Favorites,
                    compact: collapsed(),
                }

                SidebarButton {
                    screen: Route::Settings,
                    icon: "settings",
                    label: "Settings",
                    active: current_route == Route::Settings,
                    compact: collapsed(),
                }
            }

            // Dashed separator
            hr {
                class: if collapsed() {
                    "w-full mx-auto mt-4 border-t border-dashed border-outline-variant"
                } else {
                    "w-1/2 mx-auto mt-4 border-t border-dashed border-outline-variant"
                }
            }

            // Open file button
            OpenFile {
                compact: collapsed(),
            }

            // Updates block
            if !collapsed() {
                div {
                    class: "mt-auto w-[calc(100%-2rem)] max-w-[520px] mx-auto p-4
                            bg-primary rounded-2xl text-surface
                            border border-outline/10 shadow-lg
                            lg:w-full lg:max-w-none lg:mx-0",

                    p {
                        class: "text-[10px] text-on-surface font-medium mb-1 \
                                tracking-wide font-label-md text-label-md font-bold",
                        "Version {crate::shared::APP_VERSION}"
                    }

                    p {
                        class: "text-sm font-semibold mb-3 leading-tight \
                                tracking-tight text-surface-bright",
                        "New update available"
                    }

                    button {
                        class: "w-full py-2 bg-surface-bright \
                                hover:bg-surface-container-low cursor-pointer \
                                text-on-surface font-semibold rounded-xl text-xs \
                                active:scale-[0.98] transition-all flex \
                                items-center justify-center gap-2 \
                                border border-outline-variant/20 shadow-sm",

                        span {
                            class: "material-symbols-outlined text-sm \
                                    text-on-surface/90 font-normal",
                            "update"
                        }

                        "Update Now"
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
