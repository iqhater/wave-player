// For JSON store app's init config like a settings, volume, presets, theme...
use crate::shared::store_types::AppConfig;
use std::fs;
use std::path::PathBuf;

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
        if let Ok(json) = serde_json::to_string_pretty(cfg) {
            let _ = fs::write(&self.path, json);
        }
    }
}
