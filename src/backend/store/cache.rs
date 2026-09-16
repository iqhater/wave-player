// For heavy data like a waveform peaks in binary format, album covers, temp audio dumps
use crate::backend::cover_cache;
use std::fs;
use std::path::{Path, PathBuf};

pub struct CacheStore {
    dir: PathBuf,
}

impl CacheStore {
    pub fn new(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self { dir }
    }

    pub fn set(&self, key: &str, bytes: &[u8]) -> PathBuf {
        let path = self.dir.join(format!("{}.bin", key));

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let _ = fs::write(&path, bytes);
        path
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        fs::read(self.dir.join(format!("{}.bin", key))).ok()
    }

    pub fn save_cover(&self, track_path: &Path, bytes: &[u8]) -> Option<PathBuf> {
        cover_cache::save_cover(&self.dir, track_path, bytes)
    }
}
