use crate::PlayerContext;
use crate::gui::components::album_art::TrackThumbnail;
use crate::shared::command_types::{PlayerCommand, StoreCommand};
use crate::shared::event_types::{PlayerEvent, StoreEvent};

use std::path::PathBuf;

use crate::shared::store_types::Track;
use dioxus::prelude::*;
use futures_util::StreamExt;

const FAVORITES_GRID_COLUMNS: &str = "grid-cols-[40px_48px_minmax(0,1fr)_minmax(0,1fr)_40px] \
     md:grid-cols-[40px_48px_minmax(180px,2fr)_minmax(140px,1.5fr)_minmax(120px,1.5fr)_72px_72px_72px_72px_40px]";

#[component]
pub fn Favorites() -> Element {
    let ctx = use_context::<PlayerContext>();
    let mut tracks = use_signal(Vec::<Track>::new);

    let mut current_track_path = use_signal(|| None::<PathBuf>);
    let mut is_playing = use_signal(|| false);

    // Сначала создаём receiver, затем отправляем запрос.
    // Иначе FavoritesLoaded может прийти до подписки UI.
    use_future(move || {
        let mut store_rx = ctx.store_event_rx.read().clone();
        let store_cmd_tx = ctx.store_cmd_tx;

        async move {
            // println!("ui: Favorites listener started");

            match store_cmd_tx.read().send(StoreCommand::GetFavorites) {
                Ok(()) => {
                    // println!("ui: GetFavorites sent");
                }
                Err(error) => {
                    eprintln!("ui: failed to send GetFavorites: {error}");
                    return;
                }
            }

            while let Some(event) = store_rx.next().await {
                // println!("ui: received store event: {event:?}");

                match event {
                    StoreEvent::FavoritesLoaded {
                        tracks: loaded_tracks,
                    } => {
                        // println!("ui: loaded {} favorite track(s)", loaded_tracks.len());

                        tracks.set(loaded_tracks);
                    }

                    StoreEvent::FavoritesChanged {
                        track_id,
                        is_favorite,
                    } => {
                        if is_favorite {
                            // После добавления получаем обновлённый список.
                            let _ = store_cmd_tx.read().send(StoreCommand::GetFavorites);
                        } else {
                            // После удаления сразу убираем строку из UI.
                            tracks.write().retain(|track| track.id != track_id);
                        }
                    }

                    StoreEvent::TrackAnalysisUpdated { path, bpm, key } => {
                        tracks.with_mut(|tracks| {
                            if let Some(track) = tracks.iter_mut().find(|track| track.path == path)
                            {
                                track.bpm = Some(bpm);
                                track.key = Some(key);
                            }
                        });
                    }

                    StoreEvent::TrackPlaysCountUpdated { path, count } => {
                        if let Some(track) =
                            tracks.write().iter_mut().find(|track| track.path == path)
                        {
                            track.play_count = count as i64;
                        }
                    }

                    _ => {}
                }
            }

            eprintln!("ui: Favorites store event stream closed");
        }
    });

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        let command_tx = ctx.command_tx;

        async move {
            let _ = command_tx.read().send(PlayerCommand::RequestState);

            while let Some(event) = status_rx.next().await {
                match event {
                    PlayerEvent::PlayerState {
                        path,
                        is_playing: playing,
                    } => {
                        current_track_path.set(path);
                        is_playing.set(playing);
                    }

                    PlayerEvent::Started { metadata, .. } => {
                        current_track_path.set(Some(PathBuf::from(metadata.track_path)));
                        is_playing.set(true);
                    }
                    PlayerEvent::Resumed => {
                        is_playing.set(true);
                    }
                    PlayerEvent::Paused | PlayerEvent::Finished => {
                        is_playing.set(false);
                    }
                    _ => {}
                }
            }
        }
    });

    let tracks_snapshot = tracks();

    let queue_paths = tracks_snapshot
        .iter()
        .map(|track| track.path.clone())
        .collect::<Vec<_>>();

    rsx! {
        section {
            class: "w-full flex flex-col gap-3",

            div {
                class: "flex items-center justify-between px-4",

                h1 {
                    class: "text-2xl font-semibold",
                    "Favorites"
                }

                span {
                    class: "text-sm text-secondary",
                    "{tracks_snapshot.len()} tracks"
                }
            }

            // Заголовки колонок.
            div {
                class: "w-full min-w-0 flex flex-col gap-3 mt-4",
                style: "max-height: 588px; overflow-y: auto; align-content: flex-start; scrollbar-gutter: stable both-edges;",

                div {
                    class: "sticky top-0 z-10 w-full bg-surface grid {FAVORITES_GRID_COLUMNS} items-center \
                            divide-x divide-outline-variant/50 gap-2 px-3 \
                            text-xs font-semibold text-secondary uppercase",

                    div {}
                    div {}

                    div {
                        class: "min-w-0",
                        "Title"
                    }

                    div {
                        class: "min-w-0",
                        "Artist"
                    }

                    div {
                        class: "hidden md:block",
                        "Album"
                    }

                    div {
                        class: "hidden md:block text-center",
                        "BPM"
                    }

                    div {
                        class: "hidden md:block text-center",
                        "Key"
                    }

                    div {
                        class: "hidden md:block text-center",
                        "Plays"
                    }

                    div {
                        class: "hidden md:block text-center",
                        "Duration"
                    }
                    div {}
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
                    class: "w-full flex flex-col gap-3",

                    for track in tracks_snapshot {
                        FavoriteTrackRow {
                            key: "{track.id}",
                            is_playing: current_track_path() == Some(track.path.clone()) && is_playing(),
                            track,
                            on_play: {
                                let command_tx = ctx.command_tx;
                                let queue_paths = queue_paths.clone();

                                move |track: Track| {
                                    let _ = command_tx
                                        .read()
                                        .send(PlayerCommand::PlayQueue {
                                            tracks: queue_paths.clone(),
                                            current: track.path,
                                        });
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
    is_playing: bool,
    on_remove: EventHandler<i64>,
) -> Element {
    let track_id = track.id;
    let track_for_play = track.clone();

    let bpm = track
        .bpm
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "—".to_string());

    let key = track.key.clone().unwrap_or_else(|| "—".to_string());

    let album = track.album.clone().unwrap_or_else(|| "—".to_string());

    let duration = format_duration(track.duration);

    rsx! {
        div {
            class: "w-full min-w-0 grid {FAVORITES_GRID_COLUMNS} items-center divide-x divide-outline-variant/50 gap-2 px-3 py-1 rounded-2xl neumorph-raised",

            // Кнопка воспроизведения.
            button {
                class: "w-9 h-9 shrink-0 rounded-full flex items-center justify-center text-primary hover:bg-surface-container hover:scale-105 active:scale-95 transition-all cursor-pointer",
                title: if is_playing { "pause track" } else { "play track" },
                onclick: move |_| on_play.call(track_for_play.clone()),

                span {
                    class: "material-symbols-outlined text-[24px]",
                    if is_playing { "pause" } else { "play_arrow" }
                }
            }

            // Обложка или компактная заглушка.
            TrackThumbnail {
                cache_path: track.cover_cache_path.clone(),
            }

            // Название.
            div {
                class: "min-w-0 overflow-hidden",

                p {
                    class: "truncate font-small",
                    title: "{track.title}",
                    "{track.title}"
                }
            }

            // Исполнитель.
            div {
                class: "min-w-0 overflow-hidden truncate text-secondary text-sm",
                title: "{track.artist}",
                "{track.artist}"
            }

            // Альбом.
            div {
                class: "hidden md:block min-w-0 truncate text-secondary opacity-70 text-sm",
                title: "{album}",
                "{album}"
            }

            // BPM.
            div {
                class: "hidden md:block text-center text-sm",
                "{bpm}"
            }

            // Тональность.
            div {
                class: "hidden md:block text-center text-sm",
                "{key}"
            }

            // Количество прослушиваний.
            div {
                class: "hidden md:block text-center text-sm",
                "{track.play_count}"
            }

            // Длительность.
            div {
                class: "hidden md:block text-center text-secondary text-sm",
                "{duration}"
            }

            // Удаление.
            button {
                class: "w-9 h-9 shrink-0 rounded-full flex items-center justify-center text-secondary hover:text-error hover:bg-surface-container cursor-pointer transition-all",
                title: "remove from favorites",
                onclick: move |_| on_remove.call(track_id),

                span {
                    class: "material-symbols-outlined text-[20px]",
                    "delete"
                }
            }
        }
    }
}

fn format_duration(duration_ms: i64) -> String {
    let total_seconds = (duration_ms.max(0) / 1000) as u64;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;

    format!("{minutes}:{seconds:02}")
}
