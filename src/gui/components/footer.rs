use crate::gui::route::Route;
use dioxus::prelude::*;

#[component]
pub fn Footer() -> Element {
    rsx! {
        footer { class: "pointer-events-none fixed bottom-0 z-10
                    hidden h-16 w-full items-center justify-between
                    bg-transparent px-container-padding
                    lg:flex",
            div { class: "flex items-center gap-stack-md pointer-events-auto" }

            div { class: "flex gap-stack-md pointer-events-auto",
                Link { class: "font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all", to: Route::TermsOfService, "Terms Of Service" }
                Link { class: "font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all", to: Route::PrivacyPolicy, "Privacy Policy" }
                a { class: "flex items-center font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all",
                    href: "https://github.com/iqhater/wave-player",
                    span { class: "material-symbols-outlined text-[24px]", "code" } "Github Repo" }
                a { class: "flex items-center font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all",
                    href: "https://github.com/iqhater/wave-player/issues/new",
                    span { class: "material-symbols-outlined text-[24px]", "bug_report" } "Bug Report" }
                Link {
                    class: "flex items-center font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all",
                    to: Route::Changelog,
                    span {
                        class: "material-symbols-outlined text-[24px]",
                        "history"
                    }
                    "Changelog"
                }
                a { class: "flex items-center font-label-sm text-label-sm text-secondary opacity-60 hover:text-primary transition-all",
                    href: "https://www.donationalerts.com/r/iqhater",
                    span { class: "material-symbols-outlined text-[24px]", "attach_money" } "Support App"
                }
            }
        }
    }
}
