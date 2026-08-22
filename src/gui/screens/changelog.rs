use dioxus::prelude::*;

const STYLES_CSS: Asset = asset!("/src/gui/screens/markdown_styles.css");

#[component]
pub fn Changelog() -> Element {
    let html = use_memo(move || {
        let markdown = include_str!("../../../CHANGELOG.md");

        markdown::to_html_with_options(markdown, &markdown::Options::gfm())
            .unwrap_or_else(|error| format!("<p>Error rendering changelog: {error}</p>"))
    });

    rsx! {
        document::Stylesheet { href: STYLES_CSS }

        div {
            class: "markdown-content prose prose-sm max-w-none text-on-surface w-full h-full overflow-y-auto",
            style: "height: calc(100vh - 180px); overflow-y: auto; scrollbar-width: none; -ms-overflow-style: none; width: 100%; max-width: 800px; margin: 0 auto;",
            dangerous_inner_html: html(),
        }
    }
}
