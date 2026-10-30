use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{StoreEvent, StoreSender};
use std::path::PathBuf;

use super::cache::CacheStore;
use super::config::ConfigStore;
use super::db::DbStore;
use crate::backend::store::dispatch;

/// Отдельный поток: независимо пишет в db/cache/config и шлёт события в UI.
/// UI никогда не ждёт ответа — всё асинхронно через каналы.
pub fn run_store_loop(
    store_rx: flume::Receiver<StoreCommand>,
    store_tx: StoreSender,
    data_dir: PathBuf,
    app_config: ConfigStore,
) -> Result<(), Box<dyn std::error::Error>> {
    std::thread::spawn(move || {
        let mut db = match DbStore::open(&data_dir.join("wave_player.db")) {
            // let mut db = match DbStore::open(&PathBuf::from("D:/Web_Dev/Dev/Rust/wave_player/db/wave_player.db")) {
            Ok(db) => db,
            Err(error) => {
                eprintln!("failed to open database: {error:#?}");
                return;
            }
        };
        let cache = CacheStore::new(data_dir.join("cache"));

        // Keep the config in memory so analysis updates can honor manual BPM
        // overrides without racing an asynchronous config file write.
        let mut current_config = app_config.load();

        // При старте сразу отдаём настройки в UI
        let _ = store_tx.try_broadcast(StoreEvent::ConfigLoaded {
            config: current_config.clone(),
        });

        while let Ok(command) = store_rx.recv() {
            // run store commands
            dispatch::dispatch(
                &mut db,
                &app_config,
                &mut current_config,
                &cache,
                &store_tx,
                command,
            );
        }
    });
    Ok(())
}
