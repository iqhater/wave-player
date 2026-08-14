use crate::PlayerContext;
use crate::gui::theme::{Theme, use_theme};
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::StoreEvent;
use crate::shared::store_types::AppConfig;
use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn ThemeToggle() -> Element {
    let theme = use_theme();
    let ctx = use_context::<PlayerContext>();

    rsx! {
        div { class: "absolute top-8 right-8 z-50",
            button {
                class: "w-12 h-12 rounded-full glass-toggle flex items-center justify-center text-secondary cursor-pointer hover:text-primary hover:scale-110 transition-all duration-300",
                "aria-label": "Toggle Theme",
                onclick: {
                    move |_| {
                        theme.toggle();
                            let next_theme = theme.get();

                            // Клонируем весь конфиг целиком, это безопасно
                            let mut current_config = ctx.config.read().clone();

                            current_config.theme = next_theme.to_pref();

                            *ctx.config.write() = current_config.clone();

                            let _ = ctx.store_cmd_tx.read().send(StoreCommand::SetConfig(current_config));
                    }
                },
                span { class: "material-symbols-outlined text-[24px]",
                    if theme.get() == Theme::Dark { "light_mode" } else { "dark_mode" }
                }
            }
        }
    }
}
