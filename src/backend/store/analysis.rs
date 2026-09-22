use super::db::DbStore;
use std::path::Path;

impl DbStore {
    pub fn update_track_analysis(
        &mut self,
        path: &Path,
        bpm: f32,
        key: &str,
    ) -> rusqlite::Result<bool> {
        let updated = self.conn.execute(
            "UPDATE tracks
                 SET bpm = ?1,
                     key = ?2
                 WHERE path = ?3",
            rusqlite::params![bpm, key, path.to_string_lossy().as_ref()],
        )?;

        Ok(updated > 0)
    }
}
