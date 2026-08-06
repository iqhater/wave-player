use crate::gui::route::Route;
use dioxus::prelude::*;

#[component]
pub fn Footer() -> Element {
    rsx! {
        footer { class: "fixed bottom-0 w-full h-16 z-10 flex items-center justify-between px-container-padding bg-transparent pointer-events-none",
            div { class: "flex items-center gap-stack-md pointer-events-auto" }

            div { class: "flex gap-stack-md pointer-events-auto",
                Link { class: "font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all", to: Route::TermsOfService, "Terms Of Service" }
                Link { class: "font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all", to: Route::PrivacyPolicy, "Privacy Policy" }
                a { class: "font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all", href: "#", "Github" }
                a { class: "font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all", href: "#", "Bug Report" }
                a { class: "font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all", href: "#", "Support" }
            }
        }
    }
}
