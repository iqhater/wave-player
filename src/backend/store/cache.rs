// For heavy data like a waveform peaks in binary format, album covers, temp audio dumps
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
        let p = self.dir.join(format!("{}.bin", key));
        let _ = fs::write(&p, bytes);
        p
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        fs::read(self.dir.join(format!("{}.bin", key))).ok()
    }
}
