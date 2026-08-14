use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{PlayerEvent, StoreEvent, StoreSender};
use std::path::PathBuf;

use super::cache::CacheStore;
use super::config::ConfigStore;
use super::db::DbStore;
use crate::shared::store_types::AppConfig;

/// Отдельный поток: независимо пишет в db/cache/config и шлёт события в UI.
/// UI никогда не ждёт ответа — всё асинхронно через каналы.
pub fn run_store_loop(
    store_rx: flume::Receiver<StoreCommand>,
    store_tx: StoreSender,
    data_dir: PathBuf,
    app_config: ConfigStore,
) -> Result<(), Box<dyn std::error::Error>> {
    std::thread::spawn(move || {
        let mut db = DbStore::open(&data_dir.join("wave_player.db"));
        let cache = CacheStore::new(data_dir.join("cache"));

        // При старте сразу отдаём настройки в UI
        let _ = store_tx.broadcast(StoreEvent::ConfigLoaded {
            config: app_config.load(),
        });

        while let Ok(cmd) = store_rx.recv() {
            match cmd {
                StoreCommand::SaveTrackPlaysCount { path } => {
                    db.increment_play_count(&path);
                    // Послать обновлённый счётчик обратно
                    if let Some(count) = db.play_count(&path) {
                        let _ =
                            store_tx.broadcast(StoreEvent::TrackPlaysCountUpdated { path, count });
                    }
                }
                StoreCommand::UpdateTrackInfo {
                    path,
                    duration_secs,
                } => {
                    db.edit_track(&path, duration_secs);
                }
                StoreCommand::SetConfig(cfg) => {
                    app_config.save(&cfg);
                }
                StoreCommand::RequestSnapshot => {
                    let _ = store_tx.broadcast(StoreEvent::ConfigLoaded {
                        config: app_config.load(),
                    });
                    let _ = store_tx.broadcast(StoreEvent::FavoritesLoaded {
                        tracks: db.favorites(),
                    });
                }
            }
        }
    });
    Ok(())
}
