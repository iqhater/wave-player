use crate::PlayerContext;
use crate::shared::command_types::{PlayerCommand, StoreCommand};
use crate::shared::event_types::{PlayerEvent, StoreEvent};
use crate::shared::store_types::{RecentTrack, Track};
use chrono::{Local, NaiveDateTime, TimeZone};

use dioxus::prelude::*;
use futures_util::StreamExt;
use std::path::PathBuf;

const RECENT_GRID_COLUMNS: &str = "grid-cols-[40px_48px_minmax(0,1fr)_minmax(0,1fr)_40px] \
     md:grid-cols-[40px_48px_minmax(180px,2fr)_minmax(140px,1.5fr)_minmax(180px,1.5fr)_160px_72px_40px]";

#[component]
pub fn RecentTracks() -> Element {
    let ctx = use_context::<PlayerContext>();

    let mut tracks = use_signal(Vec::<RecentTrack>::new);
    let mut current_track_path = use_signal(|| None::<PathBuf>);
    let mut is_playing = use_signal(|| false);

    use_future(move || {
        let mut store_rx = ctx.store_event_rx.read().clone();
        let store_cmd_tx = ctx.store_cmd_tx;

        async move {
            let _ = store_cmd_tx.read().send(StoreCommand::GetRecentTracks);

            while let Some(event) = store_rx.next().await {
                match event {
                    StoreEvent::RecentTracksLoaded {
                        tracks: loaded_tracks,
                    } => {
                        tracks.set(loaded_tracks);
                    }

                    StoreEvent::RecentTrackChanged { .. } => {
                        let _ = store_cmd_tx.read().send(StoreCommand::GetRecentTracks);
                    }

                    _ => {}
                }
            }
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

    rsx! {
        section {
            class: "w-full flex flex-col gap-3",

            div {
                class: "flex items-center justify-between px-4",

                h1 {
                    class: "text-2xl font-semibold",
                    "Recent Tracks"
                }

                span {
                    class: "text-sm text-secondary",
                    "{tracks_snapshot.len()} tracks"
                }
            }

            div {
                class: "w-full grid {RECENT_GRID_COLUMNS} items-center \
                        divide-x divide-outline-variant/50 gap-2 px-3 mt-4 \
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
                    class: "hidden md:block text-center whitespace-nowrap",
                    "Last played"
                }

                div {
                    class: "hidden md:block text-center",
                    "Duration"
                }

                div {}
            }

            if tracks_snapshot.is_empty() {
                div {
                    class: "min-h-48 flex flex-col items-center justify-center \
                            gap-3 rounded-3xl neumorph-raised text-secondary",

                    span {
                        class: "material-symbols-outlined text-5xl",
                        "history"
                    }

                    p {
                        "Your recent tracks list is empty"
                    }

                    p {
                        class: "text-sm opacity-70",
                        "Opened tracks will appear here"
                    }
                }
            } else {
                div {
                    class: "w-full flex flex-col gap-3",

                    for recent_track in tracks_snapshot {
                        RecentTrackRow {
                            key: "{recent_track.track.id}",

                            recent_track: recent_track.clone(),

                            is_playing: current_track_path()
                                == Some(recent_track.track.path.clone())
                                && is_playing(),
                            on_play: {
                                let command_tx = ctx.command_tx;

                                move |track: Track| {
                                    let is_current_track =
                                        current_track_path()
                                            == Some(track.path.clone());

                                    let command = if is_current_track {
                                        if is_playing() {
                                            PlayerCommand::Pause
                                        } else {
                                            PlayerCommand::Resume
                                        }
                                    } else {
                                        PlayerCommand::LoadFile(
                                            track.path
                                                .to_string_lossy()
                                                .into_owned(),
                                        )
                                    };

                                    let _ = command_tx.read().send(command);
                                }
                            },
                            on_remove: {
                                let store_cmd_tx = ctx.store_cmd_tx;

                                move |track_id: i64| {
                                    let _ = store_cmd_tx
                                        .read()
                                        .send(
                                            StoreCommand::RemoveRecentTrack {
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
fn RecentTrackRow(
    recent_track: RecentTrack,
    is_playing: bool,
    on_play: EventHandler<Track>,
    on_remove: EventHandler<i64>,
) -> Element {
    let track = recent_track.track;
    let track_id = track.id;
    let track_for_play = track.clone();

    let album = track.album.clone().unwrap_or_else(|| "—".to_string());

    let duration = format_duration(track.duration);
    let played_at = format_played_at(&recent_track.played_at);

    rsx! {
        div {
            class: "w-full min-w-0 grid {RECENT_GRID_COLUMNS} items-center \
                    divide-x divide-outline-variant/50 gap-2 px-3 py-1 \
                    rounded-2xl neumorph-raised",

            button {
                class: "w-9 h-9 shrink-0 rounded-full flex items-center \
                        justify-center text-primary hover:bg-surface-container \
                        hover:scale-105 active:scale-95 transition-all cursor-pointer",

                title: if is_playing {
                    "pause track"
                } else {
                    "play track"
                },

                onclick: move |_| on_play.call(track_for_play.clone()),

                span {
                    class: "material-symbols-outlined text-[24px]",
                    if is_playing { "pause" } else { "play_arrow" }
                }
            }

            div {
                class: "w-10 h-10 shrink-0 rounded-lg overflow-hidden \
                        bg-surface-container flex items-center justify-center",

                span {
                    class: "material-symbols-outlined text-[20px] text-secondary",
                    "music_note"
                }
            }

            div {
                class: "min-w-0 overflow-hidden",

                p {
                    class: "truncate font-small",
                    title: "{track.title}",
                    "{track.title}"
                }
            }

            div {
                class: "min-w-0 overflow-hidden truncate text-secondary text-sm",
                title: "{track.artist}",
                "{track.artist}"
            }

            div {
                class: "hidden md:block min-w-0 truncate text-secondary \
                        opacity-70 text-sm",
                title: "{album}",
                "{album}"
            }

            div {
                class: "hidden md:block text-center text-sm whitespace-nowrap",
                "{played_at}"
            }

            div {
                class: "hidden md:block text-center text-secondary text-sm",
                "{duration}"
            }

            button {
                class: "w-9 h-9 shrink-0 rounded-full flex items-center \
                        justify-center text-secondary hover:text-error \
                        hover:bg-surface-container cursor-pointer transition-all",

                title: "remove from recent tracks",

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

fn format_played_at(value: &str) -> String {
    let Ok(utc_datetime) = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") else {
        return value.to_string();
    };

    let utc_datetime = chrono::Utc.from_utc_datetime(&utc_datetime);
    let local_datetime = utc_datetime.with_timezone(&Local);

    local_datetime.format("%d.%m.%Y %H:%M").to_string()
}
