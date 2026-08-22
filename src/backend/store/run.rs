use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{PlayerEvent, StoreEvent, StoreSender};
use std::path::PathBuf;

use super::cache::CacheStore;
use super::config::ConfigStore;
use super::db::DbStore;
use crate::backend::store::dispatch;
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

        while let Ok(command) = store_rx.recv() {
            // run store commands
            dispatch::dispatch(&mut db, &app_config, &store_tx, command);
        }
    });
    Ok(())
}
