use super::cache::CacheStore;
use crate::shared::command_types::StoreCommand;

use crate::shared::event_types::{StoreEvent, StoreSender};
use crate::shared::store_types::AppConfig;

use super::analysis::should_apply_detected_bpm;
use super::config::ConfigStore;
use super::db::DbStore;

pub(crate) fn dispatch(
    db: &mut DbStore,
    config: &ConfigStore,
    app_config: &mut AppConfig,
    cache: &CacheStore,
    store_tx: &StoreSender,
    command: StoreCommand,
) {
    match command {
        StoreCommand::SetConfig(config_data) => {
            config.save(&config_data);
            *app_config = config_data;
        }

        StoreCommand::RequestSnapshot => {
            let _ = store_tx.broadcast(StoreEvent::ConfigLoaded {
                config: app_config.clone(),
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
            let cover_cache_path = cache.save_cover(&path, &track_data.cover);

            match db.add_favorite(&path, &track_data, cover_cache_path.as_deref()) {
                Ok(track_id) => {
                    match store_tx.try_broadcast(StoreEvent::FavoritesChanged {
                        track_id,
                        is_favorite: true,
                    }) {
                        Ok(_) => {
                            // println!("backend: favorite added, track_id={track_id}");
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
                        // println!("backend: favorite removed, track_id={track_id}");
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

            // println!("backend: loaded {} favorite track(s)", tracks.len());
            // println!("backend: favorite track(s) {:?}", tracks);

            match store_tx.try_broadcast(StoreEvent::FavoritesLoaded { tracks }) {
                Ok(_) => {
                    // println!("backend: FavoritesLoaded sent");
                }
                Err(error) => {
                    eprintln!("backend: failed to broadcast FavoritesLoaded: {error:?}");
                }
            }
        }

        StoreCommand::UpdateTrackAnalysis { path, bpm, key } => {
            if should_apply_detected_bpm(app_config, &path) {
                match db.update_track_analysis(&path, bpm, &key) {
                    Ok(true) => {
                        let _ = store_tx.try_broadcast(StoreEvent::TrackAnalysisUpdated {
                            path,
                            bpm,
                            key,
                        });
                    }
                    Ok(false) => {
                        eprintln!("track not found: {}", path.display());
                    }
                    Err(error) => {
                        eprintln!("failed to update track analysis: {error}");
                    }
                }
            } else {
                let path_string = path.to_string_lossy().into_owned();
                let manual_bpm = app_config
                    .ui
                    .manual_grids
                    .get(&path_string)
                    .and_then(|grid| grid.tempo)
                    .map(f32::round)
                    .unwrap_or(bpm);

                match db.update_track_key(&path, &key) {
                    Ok(true) => {
                        let _ = store_tx.try_broadcast(StoreEvent::TrackAnalysisUpdated {
                            path,
                            bpm: manual_bpm,
                            key,
                        });
                    }
                    Ok(false) => {
                        eprintln!("track not found: {}", path.display());
                    }
                    Err(error) => {
                        eprintln!("failed to update track key: {error}");
                    }
                }
            }
        }

        StoreCommand::SetTrackBpm { path, bpm } => match db.update_track_bpm(&path, bpm) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!("track not found: {}", path.display());
            }
            Err(error) => {
                eprintln!("failed to update manual BPM: {error}");
            }
        },

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
            let cover_cache_path = cache.save_cover(&path, &track_data.cover);

            match db.record_recent_track(&path, &track_data, cover_cache_path.as_deref()) {
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

        StoreCommand::LoadCachedAnalysis { path, reply } => {
            let result = db
                .load_cached_analysis(&path)
                .map_err(|error| error.to_string());

            if let Err(error) = &result {
                eprintln!(
                    "failed to load analysis metadata for {}: {error}",
                    path.display()
                );
            }

            let _ = reply.try_send(result);
        }

        StoreCommand::ClearAnalysisCache => {
            if let Err(error) = db.clear_analysis_cache() {
                eprintln!("failed to clear analysis cache: {error}");
            }
        }

        StoreCommand::SaveCachedAnalysis {
            path,
            waveform_cache_path,
            data,
        } => match db.save_cached_analysis(&path, waveform_cache_path.as_deref(), &data) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!(
                    "cannot save analysis: track is not stored: {}",
                    path.display()
                );
            }
            Err(error) => {
                eprintln!(
                    "failed to save analysis metadata for {}: {error}",
                    path.display()
                );
            }
        },

        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::dispatch;
    use crate::backend::store::cache::CacheStore;
    use crate::backend::store::config::ConfigStore;
    use crate::backend::store::db::DbStore;
    use crate::shared::command_types::StoreCommand;
    use crate::shared::store_types::{AppConfig, ManualGrid};
    use async_broadcast::broadcast;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "wave_player_dispatch_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ));
        std::fs::create_dir_all(&directory).expect("create temp dispatch dir");
        directory
    }

    fn database_with_track(directory: &Path) -> DbStore {
        let db = DbStore::open(&directory.join("test.db")).expect("open test database");
        db.conn
            .execute(
                "INSERT INTO tracks (path, file_size, duration)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params!["music/track.wav", 100, 1000],
            )
            .expect("insert test track");
        db
    }

    fn read_bpm_key(db: &DbStore) -> (Option<f32>, Option<String>) {
        db.conn
            .query_row(
                "SELECT bpm, key FROM tracks WHERE path = ?1",
                ["music/track.wav"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read track analysis")
    }

    #[test]
    fn detected_bpm_should_be_stored_without_a_manual_override() {
        let directory = temp_dir("detected");
        let mut db = database_with_track(&directory);
        let config_store = ConfigStore::new(directory.join("config.json"));
        let cache = CacheStore::new(directory.join("cache"));
        let (store_tx, _store_rx) = broadcast(16);
        let mut app_config = AppConfig::default();

        dispatch(
            &mut db,
            &config_store,
            &mut app_config,
            &cache,
            &store_tx,
            StoreCommand::UpdateTrackAnalysis {
                path: PathBuf::from("music/track.wav"),
                bpm: 124.0,
                key: "Am".to_string(),
            },
        );

        assert_eq!(read_bpm_key(&db), (Some(124.0), Some("Am".to_string())));
        drop(db);
        std::fs::remove_dir_all(directory).expect("remove temp dispatch dir");
    }

    #[test]
    fn manual_bpm_should_survive_a_detected_update() {
        let directory = temp_dir("manual");
        let mut db = database_with_track(&directory);
        let config_store = ConfigStore::new(directory.join("config.json"));
        let cache = CacheStore::new(directory.join("cache"));
        let (store_tx, _store_rx) = broadcast(16);
        let mut app_config = AppConfig::default();
        app_config.ui.manual_grids.insert(
            "music/track.wav".to_string(),
            ManualGrid {
                tempo: Some(128.0),
                anchor: None,
            },
        );

        // Simulate the UI saving a manual override, then a detected analysis
        // arriving before the next snapshot.
        let config_to_save = app_config.clone();
        dispatch(
            &mut db,
            &config_store,
            &mut app_config,
            &cache,
            &store_tx,
            StoreCommand::SetConfig(config_to_save),
        );
        dispatch(
            &mut db,
            &config_store,
            &mut app_config,
            &cache,
            &store_tx,
            StoreCommand::SetTrackBpm {
                path: PathBuf::from("music/track.wav"),
                bpm: 128.0,
            },
        );
        dispatch(
            &mut db,
            &config_store,
            &mut app_config,
            &cache,
            &store_tx,
            StoreCommand::UpdateTrackAnalysis {
                path: PathBuf::from("music/track.wav"),
                bpm: 124.0,
                key: "Am".to_string(),
            },
        );

        assert_eq!(read_bpm_key(&db), (Some(128.0), Some("Am".to_string())));
        drop(db);
        std::fs::remove_dir_all(directory).expect("remove temp dispatch dir");
    }
}
