use dioxus::prelude::*;

use super::layout::RootLayout;
use super::screens::{
    app_info::AppInfo, changelog::Changelog, favorites::Favorites, home::Home,
    privacy::PrivacyPolicy, recent_tracks::RecentTracks, settings::Settings, terms::TermsOfService,
};

#[derive(Clone, Debug, PartialEq, Routable)]
#[rustfmt::skip]
pub enum Route {
    #[layout(RootLayout)]
        // Sidebar menu
        #[route("/")]
        Home,

        #[route("/recent")]
        RecentTracks,

        #[route("/favorites")]
        Favorites,

        #[route("/settings")]
        Settings,

        #[route("/app-info")]
        AppInfo,

        // Footer links
        #[route("/terms")]
        TermsOfService,

        #[route("/privacy")]
        PrivacyPolicy,

        #[route("/changelog")]
        Changelog,
}
