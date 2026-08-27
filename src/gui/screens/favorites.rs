use crate::PlayerContext;
use crate::shared::command_types::{PlayerCommand, StoreCommand};
use crate::shared::event_types::StoreEvent;
use crate::shared::store_types::Track;
use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn Favorites() -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut tracks = use_signal(Vec::<Track>::new);

    // Запрашиваем избранные треки после монтирования экрана.
    /* use_effect(move || {
        let _ = ctx.store_cmd_tx.read().send(StoreCommand::GetFavorites);
    }); */

    // Получаем данные и изменения от store.
    use_future(move || {
        let mut store_rx = ctx.store_event_rx.read().clone();
        let store_cmd_tx = ctx.store_cmd_tx;

        async move {
            println!("ui: Favorites component mounted");

            match store_cmd_tx.read().send(StoreCommand::GetFavorites) {
                Ok(()) => println!("ui: GetFavorites sent"),
                Err(error) => {
                    eprintln!("ui: failed to send GetFavorites: {error}");
                    return;
                }
            }

            while let Some(event) = store_rx.next().await {
                match event {
                    StoreEvent::FavoritesLoaded {
                        tracks: loaded_tracks,
                    } => {
                        println!(
                            "ui: loaded {} favorite track(s) {:?}",
                            loaded_tracks.len(),
                            loaded_tracks
                        );
                        tracks.set(loaded_tracks);
                    }

                    StoreEvent::FavoritesChanged {
                        track_id,
                        is_favorite,
                    } => {
                        if is_favorite {
                            // После добавления запрашиваем полный список.
                            let _ = ctx.store_cmd_tx.read().send(StoreCommand::GetFavorites);
                        } else {
                            // После удаления сразу убираем трек из локального списка.
                            tracks.write().retain(|track| track.id != track_id);
                        }
                    }

                    _ => {}
                }
            }
        }
    });

    let tracks_snapshot = tracks();

    rsx! {
        section {
            class: "w-full flex flex-col gap-6",

            div {
                class: "flex items-center justify-between",

                h1 {
                    class: "text-2xl font-semibold",
                    "Favorites"
                }

                span {
                    class: "text-sm text-secondary",
                    "{tracks_snapshot.len()} tracks"
                }
            }

            if tracks_snapshot.is_empty() {
                div {
                    class: "min-h-48 flex flex-col items-center justify-center gap-3 rounded-3xl neumorph-raised text-secondary",

                    span {
                        class: "material-symbols-outlined text-5xl",
                        "favorite_border"
                    }

                    p {
                        "Your favorites list is empty"
                    }

                    p {
                        class: "text-sm opacity-70",
                        "Add tracks to favorites from the player"
                    }
                }
            } else {
                div {
                    class: "flex flex-col gap-3",

                    for track in tracks_snapshot {
                        FavoriteTrackRow {
                            key: "{track.id}",
                            track,
                            on_play: {
                                let command_tx = ctx.command_tx;
                                move |track: Track| {
                                    let _ = command_tx
                                        .read()
                                        .send(PlayerCommand::LoadFile(
                                            track.path.to_string_lossy().into_owned(),
                                        ));
                                }
                            },
                            on_remove: {
                                let store_cmd_tx = ctx.store_cmd_tx;
                                move |track_id: i64| {
                                    let _ = store_cmd_tx
                                        .read()
                                        .send(
                                            StoreCommand::RemoveFromFavorites {
                                                track_id,
                                            },
                                        );
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FavoriteTrackRow(
    track: Track,
    on_play: EventHandler<Track>,
    on_remove: EventHandler<i64>,
) -> Element {
    let track_id = track.id;
    let track_path = track.path.to_string_lossy().into_owned();

    rsx! {
        div {
            class: "w-full flex items-center gap-4 p-4 rounded-2xl neumorph-raised",

            button {
                class: "w-12 h-12 shrink-0 rounded-full flex items-center justify-center bg-primary text-on-primary hover:scale-105 active:scale-95 transition-all cursor-pointer",
                title: "play track",
                onclick: move |_| on_play.call(track.clone()),

                span {
                    class: "material-symbols-outlined",
                    "play_arrow"
                }
            }

            div {
                class: "min-w-0 flex-1",

                p {
                    class: "truncate font-medium",
                    "{track.title}"
                }

                p {
                    class: "truncate text-sm text-secondary",
                    "{track.artist}"
                }

                p {
                    class: "truncate text-xs text-secondary opacity-70",
                    "{track_path}"
                }
            }

            button {
                class: "w-10 h-10 shrink-0 rounded-full flex items-center justify-center text-secondary hover:text-error hover:bg-surface-container cursor-pointer transition-all",
                title: "remove from favorites",
                onclick: move |_| on_remove.call(track_id),

                span {
                    class: "material-symbols-outlined",
                    "delete"
                }
            }
        }
    }
}
