use super::player_controls::ControlIconButton;
use std::path::PathBuf;

use crate::PlayerContext;
use crate::gui::layout::PlaybackUIState;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::StoreEvent;
use crate::shared::store_types::{Track, TrackData};

use dioxus::prelude::*;
use futures_util::StreamExt;

#[derive(Clone, Debug, Default, PartialEq)]
struct FavoriteSelection {
    path: Option<PathBuf>,
    track_id: Option<i64>,
    is_favorite: bool,
}

impl FavoriteSelection {
    fn reset(&mut self, path: Option<PathBuf>) {
        self.path = path;
        self.track_id = None;
        self.is_favorite = false;
    }

    fn apply_loaded(&mut self, tracks: &[Track]) {
        let Some(path) = self.path.as_ref() else {
            self.track_id = None;
            self.is_favorite = false;
            return;
        };

        match tracks.iter().find(|track| &track.path == path) {
            Some(track) => {
                self.track_id = Some(track.id);
                self.is_favorite = true;
            }
            None => {
                self.track_id = None;
                self.is_favorite = false;
            }
        }
    }

    /// Returns true when the event belongs to the current track but the id is
    /// not known yet, so the caller must request a fresh favorites snapshot.
    fn apply_changed(&mut self, track_id: i64, is_favorite: bool) -> bool {
        if self.track_id == Some(track_id) {
            self.is_favorite = is_favorite;
            return false;
        }

        is_favorite && self.track_id.is_none()
    }
}

#[component]
pub fn FavoritesButton() -> Element {
    let ctx = use_context::<PlayerContext>();
    let playback = use_context::<PlaybackUIState>();
    let mut selection = use_signal(FavoriteSelection::default);

    let current_path = use_memo(move || {
        playback
            .metadata
            .read()
            .as_ref()
            .map(|track| PathBuf::from(&track.track_path))
    });

    // Restore the snapshot after every mount and whenever the current track
    // changes. This is what makes the indicator survive route changes.
    use_effect(move || {
        let path = current_path();
        let has_track = path.is_some();
        selection.write().reset(path);

        if has_track {
            let _ = ctx.store_cmd_tx.read().send(StoreCommand::GetFavorites);
        }
    });

    // Create the receiver before the initial request, so the first snapshot is
    // not lost if it arrives immediately.
    use_future(move || {
        let mut store_rx = ctx.store_event_rx.read().clone();
        let store_cmd_tx = ctx.store_cmd_tx;

        async move {
            let _ = store_cmd_tx.read().send(StoreCommand::GetFavorites);

            while let Some(event) = store_rx.next().await {
                match event {
                    StoreEvent::FavoritesLoaded { tracks } => {
                        selection.write().apply_loaded(&tracks);
                    }

                    StoreEvent::FavoritesChanged {
                        track_id,
                        is_favorite,
                    } => {
                        let needs_refresh = selection.write().apply_changed(track_id, is_favorite);

                        if needs_refresh {
                            let _ = store_cmd_tx.read().send(StoreCommand::GetFavorites);
                        }
                    }

                    _ => {}
                }
            }
        }
    });

    let is_favorite = selection().is_favorite;

    rsx! {
        ControlIconButton {
            icon: "favorite",
            title: if is_favorite {
                "remove from favorites".to_string()
            } else {
                "add to favorites".to_string()
            },
            active: is_favorite,
            click_handler: move |_| {
                let Some(path) = selection().path else {
                    return;
                };

                if selection().is_favorite {
                    let Some(track_id) = selection().track_id else {
                        return;
                    };

                    selection.write().is_favorite = false;

                    let _ = ctx
                        .store_cmd_tx
                        .read()
                        .send(StoreCommand::RemoveFromFavorites { track_id });
                } else {
                    let Some(metadata) = playback.metadata.read().clone() else {
                        return;
                    };

                    let track_data = TrackData {
                        title: metadata.title,
                        artist: metadata.artist,
                        album: (!metadata.album.is_empty()).then_some(metadata.album),
                        genre: (!metadata.genre.is_empty()).then_some(metadata.genre),
                        duration: i64::try_from(
                            ((playback.duration_secs)() * 1000.0).max(0.0) as i64,
                        )
                        .unwrap_or(i64::MAX),
                        cover: metadata.cover,
                    };

                    selection.write().is_favorite = true;

                    let _ = ctx
                        .store_cmd_tx
                        .read()
                        .send(StoreCommand::AddToFavorites { path, track_data });
                }
            },
        }
    }
}

#[cfg(test)]
mod favorites_tests {
    use super::FavoriteSelection;
    use crate::shared::store_types::Track;
    use std::path::PathBuf;

    fn track(id: i64, path: &str) -> Track {
        Track {
            id,
            path: PathBuf::from(path),
            ..Track::default()
        }
    }

    #[test]
    fn reset_should_clear_previous_state() {
        let mut selection = FavoriteSelection {
            path: Some(PathBuf::from("a.wav")),
            track_id: Some(1),
            is_favorite: true,
        };

        selection.reset(Some(PathBuf::from("b.wav")));

        assert_eq!(selection.path, Some(PathBuf::from("b.wav")));
        assert_eq!(selection.track_id, None);
        assert!(!selection.is_favorite);
    }

    #[test]
    fn loaded_should_resolve_favorite_by_path_not_by_previous_id() {
        let mut selection = FavoriteSelection {
            path: Some(PathBuf::from("b.wav")),
            track_id: Some(1),
            is_favorite: true,
        };

        selection.apply_loaded(&[track(2, "b.wav"), track(3, "c.wav")]);

        assert_eq!(selection.track_id, Some(2));
        assert!(selection.is_favorite);
    }

    #[test]
    fn loaded_should_clear_state_when_track_is_not_favorite() {
        let mut selection = FavoriteSelection {
            path: Some(PathBuf::from("b.wav")),
            track_id: Some(1),
            is_favorite: true,
        };

        selection.apply_loaded(&[track(2, "c.wav")]);

        assert_eq!(selection.track_id, None);
        assert!(!selection.is_favorite);
    }

    #[test]
    fn changed_should_update_only_the_current_track_id() {
        let mut selection = FavoriteSelection {
            path: Some(PathBuf::from("a.wav")),
            track_id: Some(1),
            is_favorite: true,
        };

        assert!(!selection.apply_changed(2, false));
        assert!(selection.is_favorite);
        assert_eq!(selection.track_id, Some(1));

        assert!(!selection.apply_changed(1, false));
        assert!(!selection.is_favorite);
    }

    #[test]
    fn changed_add_should_request_refresh_when_id_is_unknown() {
        let mut selection = FavoriteSelection {
            path: Some(PathBuf::from("a.wav")),
            track_id: None,
            is_favorite: false,
        };

        assert!(selection.apply_changed(10, true));
        assert!(!selection.apply_changed(10, false));
    }
}
