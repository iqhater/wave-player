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
            if let Err(error) = db.update_track_analysis(&path, bpm, &key) {
                eprintln!("failed to update analysis for {}: {error}", path.display());
            }
        }

        _ => {}
    }
}
