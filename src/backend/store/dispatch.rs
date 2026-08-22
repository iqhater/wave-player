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
        StoreCommand::SaveTrackPlaysCount { path } => {
            db.increment_play_count(&path);

            if let Some(count) = db.play_count(&path) {
                let _ = store_tx.broadcast(StoreEvent::TrackPlaysCountUpdated { path, count });
            }
        }

        StoreCommand::UpdateTrackInfo {
            path,
            duration_secs,
        } => {
            db.edit_track(&path, duration_secs);
        }

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
        StoreCommand::AddToFavorites { path } => {
            db.add_favorite(&path);
            let _ = store_tx.broadcast(StoreEvent::FavoritesChanged {
                path,
                is_favorite: true,
            });
        }

        StoreCommand::RemoveFromFavorites { path } => {
            db.remove_favorite(&path);
            let _ = store_tx.broadcast(StoreEvent::FavoritesChanged {
                path,
                is_favorite: false,
            });
        }

        StoreCommand::GetFavorites => {
            let _ = store_tx.broadcast(StoreEvent::FavoritesLoaded {
                tracks: db.favorites(),
            });
        }
    }
}
