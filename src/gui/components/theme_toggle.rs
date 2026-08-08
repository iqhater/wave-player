use crate::gui::theme::{Theme, use_theme};
use dioxus::prelude::*;

#[component]
pub fn ThemeToggle() -> Element {
    let theme = use_theme();

    rsx! {
        div { class: "absolute top-8 right-8 z-50",
            button {
                class: "w-12 h-12 rounded-full glass-toggle flex items-center justify-center text-secondary cursor-pointer hover:text-primary hover:scale-110 transition-all duration-300",
                "aria-label": "Toggle Theme",
                onclick:  move |_| theme.toggle(),
                span { class: "material-symbols-outlined text-[24px]",
                    if theme.get() == Theme::Dark { "light_mode" } else { "dark_mode" }
                }
            }
        }
    }
}
