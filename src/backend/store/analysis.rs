use super::db::DbStore;
use crate::shared::store_types::Track;
use crate::shared::store_types::TrackData;
use std::path::{Path, PathBuf};

impl DbStore {
    pub fn update_track_analysis(
        &mut self,
        path: &Path,
        bpm: f32,
        key: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tracks
             SET bpm = ?1,
                 key = ?2
             WHERE path = ?3",
            rusqlite::params![bpm, key, path.to_string_lossy().as_ref(),],
        )?;

        Ok(())
    }
}
