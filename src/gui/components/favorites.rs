use super::player_controls::ControlIconButton;
use std::time::Duration;

use crate::PlayerContext;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{PlayerEvent, StoreEvent};
use crate::shared::store_types::TrackData;

use dioxus::prelude::*;
use futures_util::StreamExt;
use std::path::PathBuf;

#[component]
pub fn FavoritesButton() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut current_path = use_signal(|| None::<PathBuf>);
    let mut current_track_id = use_signal(|| None::<i64>);
    let mut is_favorite = use_signal(|| false);

    // track  metadata
    let mut title = use_signal(|| "".to_string());
    let mut artist = use_signal(|| "".to_string());
    let mut album = use_signal(|| "".to_string());
    let mut genre = use_signal(|| "".to_string());
    let mut duration = use_signal(|| Duration::ZERO);
    let mut cover = use_signal(Vec::<u8>::new);

    // Получаем путь текущего трека из PlayerEvent::Started.
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();

        async move {
            while let Some(event) = status_rx.next().await {
                if let PlayerEvent::Started {
                    duration: d,
                    metadata,
                } = event
                {
                    let path = PathBuf::from(metadata.track_path);

                    current_path.set(Some(path));

                    // До ответа базы состояние неизвестно.
                    current_track_id.set(None);
                    is_favorite.set(false);

                    // set metadata
                    title.set(metadata.title);
                    artist.set(metadata.artist);
                    album.set(metadata.album);
                    genre.set(metadata.genre);
                    duration.set(d);
                    cover.set(metadata.cover);

                    // Запрашиваем актуальное избранное для нового трека.
                    let _ = ctx.store_cmd_tx.read().send(StoreCommand::GetFavorites);
                }
            }
        }
    });

    // Получаем состояние избранного из store.
    use_future(move || {
        let mut store_rx = ctx.store_event_rx.read().clone();

        async move {
            while let Some(event) = store_rx.next().await {
                match event {
                    StoreEvent::FavoritesChanged {
                        track_id,
                        is_favorite: value,
                    } => {
                        current_track_id.set(Some(track_id));
                        is_favorite.set(value);
                    }

                    StoreEvent::FavoritesLoaded { tracks } => {
                        let Some(path) = current_path() else {
                            continue;
                        };

                        if let Some(track) = tracks.into_iter().find(|track| track.path == path) {
                            current_track_id.set(Some(track.id));
                            is_favorite.set(true);
                        } else {
                            current_track_id.set(None);
                            is_favorite.set(false);
                        }
                    }

                    _ => {}
                }
            }
        }
    });

    rsx! {
        ControlIconButton {
            icon: "favorite",
            title: if is_favorite() {
                "remove from favorites".to_string()
            } else {
                "add to favorites".to_string()
            },
            active: is_favorite(),
            click_handler: move |_| {
                if is_favorite() {
                    let Some(track_id) = current_track_id() else {
                        return;
                    };

                    is_favorite.set(false);

                    let _ = ctx
                        .store_cmd_tx
                        .read()
                        .send(StoreCommand::RemoveFromFavorites { track_id });
                } else {
                    let Some(path) = current_path() else {
                        return;
                    };

                    is_favorite.set(true);

                    let track_data = TrackData {
                        title: title(),
                        artist: artist(),
                        album: {
                            let value = album();

                            if value.is_empty() {
                                None
                            } else {
                                Some(value)
                            }
                        },
                        genre: {
                            let value = genre();

                            if value.is_empty() {
                                None
                            } else {
                                Some(value)
                            }
                        },
                        duration: i64::try_from(duration().as_millis()).unwrap_or(i64::MAX),
                        cover: cover(),
                    };

                    let _ = ctx
                        .store_cmd_tx
                        .read()
                        .send(StoreCommand::AddToFavorites { path, track_data });
                }
            },
        }
    }
}
