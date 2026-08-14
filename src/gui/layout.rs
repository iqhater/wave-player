use super::route::Route;
use dioxus::prelude::*;

use super::components::{
    errors_toast::ErrorsToast, footer::Footer, player_controls::PlayerControls, sidebar::Sidebar,
    theme_toggle::ThemeToggle,
};
use super::theme::{Theme, use_theme, use_theme_provider};
use crate::PlayerContext;
use crate::shared::event_types::StoreEvent;
use futures_util::StreamExt;

// Geist link "https://fonts.googleapis.com/css2?family=Geist:wght@100;300;400;500;600;700;800;900&display=swap"
static FAVICON: Asset = asset!("/assets/favicon.png");
// static TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
// static GOOGLE_FONTS_GEIST_LINK: &str = "https://fonts.googleapis.com/css2?family=Geist:wght@100;300;400;500;600;700;800;900&display=swap";
// static MATERIAL_SYMBOLS_OUTLINED_LINK: &str = "https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:wght,FILL@100..700,0..1&display=swap";

static FONTS_GEIST: Asset = asset!("/assets/fonts/Geist[wght].woff2");
static MATERIAL_SYMBOLS_OUTLINED: Asset =
    asset!("/assets/fonts/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].woff2");
// static CSS: Asset = asset!("/assets/main.css");

#[component]
pub fn RootLayout() -> Element {
    let ctx = use_context::<PlayerContext>();
    let route = use_route::<Route>();
    let hide_controls = matches!(route, Route::TermsOfService | Route::PrivacyPolicy);

    // 1. СРАЗУ читаем сохраненную тему из конфига при старте приложения
    let saved_theme = Theme::from_pref(ctx.config.read().theme.clone());

    use_theme_provider(saved_theme);

    let theme = use_theme();
    let body_class = if theme.get() == Theme::Dark {
        "dark bg-surface text-on-surface"
    } else {
        "bg-surface text-on-surface"
    };

    // Слушаем рантайм-обновления от бэкенда
    use_future(move || {
        let mut store_event_rx = ctx.store_event_rx.read().clone();
        async move {
            while let Some(store_event) = store_event_rx.next().await {
                match store_event {
                    StoreEvent::ConfigLoaded { config } => {
                        // Записываем через .write()
                        *ctx.config.write() = config.clone();
                        theme.set(Theme::from_pref(config.theme.clone()));
                    }
                    _ => {}
                }
            }
        }
    });

    rsx! {

        document::Link { rel: "icon", href: FAVICON }

        // Approach 2 — register a bundled font with an inline @font-face rule.
        // `{FONTS_GEIST}` interpolates to the asset's resolved URL at runtime.
        style { "@font-face {{ font-family: 'Geist'; src: url('{FONTS_GEIST}') format('woff2'); font-display: swap; }}" }
        style { "@font-face {{ font-family: 'Material Symbols Outlined'; src: url('{MATERIAL_SYMBOLS_OUTLINED}') format('woff2'); font-display: block; }}" }

        // document::Stylesheet { href: TAILWIND_CSS }


        // Custom main.css no needed as tailwind
        // document::Stylesheet { href: CSS }
        // document::Stylesheet {  href: MATERIAL_SYMBOLS_OUTLINED }

        // Container for the entire app
        body { class: "{body_class}",
            div { class: "flex h-screen w-full overflow-hidden relative",

                ThemeToggle {}

                // Sidebar menu (always seen)
                Sidebar {}

                main { class: "flex-grow flex flex-col items-center justify-center relative bg-surface p-container-padding overflow-y-auto",
                    div { class: "w-full max-w-[600px] flex flex-col items-center gap-8",

                        // Router paste here: Home, Recents, Favorites or Settings
                        Outlet::<Route> {}

                        // All errors overlay notification from audio engine
                        ErrorsToast {}
                    }
                    if !hide_controls {
                        PlayerControls {}
                    }
                }
            }

            // Footer (always seen)
            Footer {}
        }
    }
}
