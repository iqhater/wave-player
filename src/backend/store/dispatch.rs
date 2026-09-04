use crate::backend::state::AudioState;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::AudioError;
use crate::shared::event_types::{StoreEvent, StoreSender};

use super::config::ConfigStore;
use super::db::DbStore;

pub(crate) fn dispatch(
    db: &mut DbStore,
    config: &ConfigStore,
    store_tx: &StoreSender,
    command: StoreCommand,
) {
    match command {
        StoreCommand::SetConfig(config_data) => {
            config.save(&config_data);
        }

        StoreCommand::RequestSnapshot => {
            let _ = store_tx.broadcast(StoreEvent::ConfigLoaded {
                config: config.load(),
            });

            let _ = store_tx.broadcast(StoreEvent::FavoritesLoaded {
                tracks: db.favorites(),
            });

            let _ = store_tx.broadcast(StoreEvent::RecentTracksLoaded {
                tracks: db.recent_tracks(),
            });
        }

        // Favorites
        StoreCommand::AddToFavorites { path, track_data } => {
            match db.add_favorite(&path, &track_data) {
                Ok(track_id) => {
                    match store_tx.try_broadcast(StoreEvent::FavoritesChanged {
                        track_id,
                        is_favorite: true,
                    }) {
                        Ok(_) => {
                            println!("backend: favorite added, track_id={track_id}");
                        }
                        Err(error) => {
                            eprintln!("backend: failed to broadcast favorite addition: {error:?}");
                        }
                    }
                }
                Err(error) => {
                    eprintln!("failed to add track to favorites: {error}");
                }
            }
        }

        StoreCommand::RemoveFromFavorites { track_id } => match db.remove_favorite(track_id) {
            Ok(()) => {
                match store_tx.try_broadcast(StoreEvent::FavoritesChanged {
                    track_id,
                    is_favorite: false,
                }) {
                    Ok(_) => {
                        println!("backend: favorite removed, track_id={track_id}");
                    }
                    Err(error) => {
                        eprintln!("backend: failed to broadcast favorite removal: {error:?}");
                    }
                }
            }
            Err(error) => {
                eprintln!("failed to remove track {track_id} from favorites: {error}");
            }
        },

        StoreCommand::GetFavorites => {
            let tracks = db.favorites();

            println!("backend: loaded {} favorite track(s)", tracks.len());
            println!("backend: favorite track(s) {:?}", tracks);

            match store_tx.try_broadcast(StoreEvent::FavoritesLoaded { tracks }) {
                Ok(_) => {
                    println!("backend: FavoritesLoaded sent");
                }
                Err(error) => {
                    eprintln!("backend: failed to broadcast FavoritesLoaded: {error:?}");
                }
            }
        }

        StoreCommand::UpdateTrackAnalysis { path, bpm, key } => {
            match db.update_track_analysis(&path, bpm, &key) {
                Ok(true) => {
                    let _ =
                        store_tx.try_broadcast(StoreEvent::TrackAnalysisUpdated { path, bpm, key });
                }
                Ok(false) => {
                    eprintln!("track not found: {}", path.display());
                }
                Err(error) => {
                    eprintln!("failed to update track analysis: {error}");
                }
            }
        }

        StoreCommand::SaveTrackPlaysCount { path } => match db.increment_play_count(&path) {
            Ok(Some(count)) => {
                let _ = store_tx.try_broadcast(StoreEvent::TrackPlaysCountUpdated {
                    path,
                    count: count as u64,
                });
            }
            Ok(None) => {
                eprintln!(
                    "cannot increment play count: track is not stored: {}",
                    path.display()
                );
            }
            Err(error) => {
                eprintln!(
                    "failed to increment play count for {}: {error}",
                    path.display()
                );
            }
        },

        StoreCommand::RecordRecentTrack { path, track_data } => {
            match db.record_recent_track(&path, &track_data) {
                Ok(track_id) => {
                    let _ = store_tx.try_broadcast(StoreEvent::RecentTrackChanged {
                        track_id,
                        is_present: true,
                    });
                }
                Err(error) => {
                    eprintln!("failed to record recent track {}: {error}", path.display());
                }
            }
        }

        StoreCommand::GetRecentTracks => {
            let tracks = db.recent_tracks();

            let _ = store_tx.try_broadcast(StoreEvent::RecentTracksLoaded { tracks });
        }

        StoreCommand::RemoveRecentTrack { track_id } => match db.remove_recent_track(track_id) {
            Ok(()) => {
                let _ = store_tx.try_broadcast(StoreEvent::RecentTrackChanged {
                    track_id,
                    is_present: false,
                });
            }
            Err(error) => {
                eprintln!("failed to remove recent track {track_id}: {error}");
            }
        },

        _ => {}
    }
}
