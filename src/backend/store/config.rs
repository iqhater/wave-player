// For JSON store app's init config like a settings, volume, presets, theme...
use crate::shared::store_types::AppConfig;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SAVE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        Self { path }
    }

    pub fn load(&self) -> AppConfig {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn load_with_default(&self) -> AppConfig {
        if !self.path.exists() {
            let cfg = AppConfig::default();
            self.save(&cfg);
            return cfg;
        }
        self.load()
    }

    pub fn save(&self, cfg: &AppConfig) {
        let Ok(json) = serde_json::to_string_pretty(cfg) else {
            return;
        };

        let temporary = self.path.with_extension(format!(
            "json.{}.{}.tmp",
            std::process::id(),
            SAVE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));

        if fs::write(&temporary, json).is_err() {
            return;
        }

        if fs::rename(&temporary, &self.path).is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigStore;
    use crate::shared::store_types::AppConfig;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_config_path(name: &str) -> (PathBuf, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "wave_player_config_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ));
        std::fs::create_dir_all(&directory).expect("create temp config dir");
        let path = directory.join("config.json");
        (directory, path)
    }

    #[test]
    fn config_store_should_default_new_fields_from_legacy_json() {
        let (directory, path) = temp_config_path("legacy");
        std::fs::write(
            &path,
            r#"{
                "theme": "Light",
                "ui": {
                    "timeline_cursor_follows_loop": false,
                    "show_time_markers": true,
                    "loop_repeat_region": true,
                    "timeline_progress_view": false,
                    "show_beat_grid": true,
                    "beat_grid_snap": false
                }
            }"#,
        )
        .expect("write legacy config");

        let config = ConfigStore::new(path).load();

        assert_eq!(
            config.theme,
            crate::shared::store_types::ThemePreference::Light
        );
        assert!(config.ui.show_hover_cursor);
        assert!(!config.ui.sidebar_collapsed);

        std::fs::remove_dir_all(directory).expect("remove temp config dir");
    }

    #[test]
    fn config_store_should_roundtrip_new_ui_fields() {
        let (directory, path) = temp_config_path("roundtrip");
        let store = ConfigStore::new(path.clone());
        let mut config = AppConfig::default();
        config.ui.show_hover_cursor = false;
        config.ui.sidebar_collapsed = true;

        store.save(&config);
        let restored = store.load();

        assert_eq!(restored, config);
        assert!(!restored.ui.show_hover_cursor);
        assert!(restored.ui.sidebar_collapsed);

        std::fs::remove_dir_all(directory).expect("remove temp config dir");
    }
}
