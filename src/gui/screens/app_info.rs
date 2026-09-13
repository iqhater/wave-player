use dioxus::prelude::*;

#[component]
pub fn AppInfo() -> Element {
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
                            "{crate::shared::APP_VERSION}"
                        }
                    }

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
