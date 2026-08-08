use super::open_file::OpenFile;
use crate::gui::route::Route;
use dioxus::prelude::*;

#[component]
pub fn Sidebar() -> Element {
    let current_route = use_route::<Route>();

    rsx! {
        aside {
            class: "h-screen w-64 flex-shrink-0 flex flex-col py-stack-lg px-gutter bg-surface shadow-[4px_4px_12px_rgba(0,0,0,0.05)] z-20 bg-surface-container-high",

            // Header Logo
            div { class: "mb-stack-md",
                h1 { class: "font-headline-lg text-headline-lg text-primary tracking-tight font-semibold",
                    "Wave Player"
                }
                p { class: "font-label-sm text-label-sm text-secondary tracking-widest uppercase",
                    "Premium Audio"
                }
                div { class: "mt-4 border-t border-outline-variant pt-4" }
            }

            // Menu Items
            nav { class: "flex flex-col gap-unit",
                SidebarButton { screen: Route::Home, icon: "play_circle", label: "Player", active: current_route == Route::Home }
                SidebarButton { screen: Route::RecentTracks, icon: "queue_music", label: "Recent Tracks", active: current_route == Route::RecentTracks }
                SidebarButton { screen: Route::Favorites, icon: "favorite", label: "Favorites", active: current_route == Route::Favorites }
                SidebarButton { screen: Route::Settings, icon: "settings", label: "Settings", active: current_route == Route::Settings }
            }

            // Dashed separator
            hr {
                class: "w-1/2 mx-auto mt-4 border-t border-dashed border-outline-variant"
            }

            // Open file button
            OpenFile {}

            // Updates block
            div {
                class: "mt-auto p-4 bg-primary rounded-2xl text-surface border border-outline/10 shadow-lg max-w-[200px]",
                p {
                    class: "text-[10px] text-on-surface font-medium mb-1 tracking-wide font-label-md text-label-md font-bold",
                    "Version {crate::shared::APP_VERSION}"
                }
                p {
                    class: "text-sm font-semibold mb-3 leading-tight tracking-tight text-surface-bright",
                    "New update available"
                }
                button {
                    class: "w-full py-2 bg-surface-bright hover:bg-surface-container-low cursor-pointer text-on-surface font-semibold rounded-xl text-xs active:scale-[0.98] transition-all flex items-center justify-center gap-2 border border-outline-variant/20 shadow-sm",
                    span {
                        class: "material-symbols-outlined text-sm text-on-surface/90 font-normal",
                        "update"
                    }
                    "Update Now"
                }
            }
        }
    }
}

#[component]
fn SidebarButton(screen: Route, icon: String, label: String, active: bool) -> Element {
    let active_class = if active {
        "flex items-center gap-4 py-3 px-4 rounded-xl text-primary font-bold border-r-4 border-primary cursor-pointer bg-surface-container-low transition-all"
    } else {
        "flex items-center gap-4 py-3 px-4 rounded-xl text-secondary font-medium hover:bg-surface-container-low cursor-pointer transition-colors active:scale-95 duration-150"
    };

    rsx! {
        Link { class: "w-full", to: screen,
            button { class: "{active_class} w-full",

                span { class: "material-symbols-outlined", "{icon}" }
                span { class: "font-body-md text-body-md", "{label}" }
            },
        }
    }
}
