use crate::gui::components::{
    album_art::AlbumArt, bpm_key::BpmKey, track_info::TrackInfo, waveform::Waveform,
};
use dioxus::prelude::*;

#[component]
pub fn Home() -> Element {
    rsx! {
        AlbumArt {}
        TrackInfo {}
        BpmKey {}
        Waveform {}
    }
}
