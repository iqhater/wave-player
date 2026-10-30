use super::db::DbStore;
use crate::shared::command_types::CachedTrackAnalysis;
use crate::shared::store_types::AppConfig;
use rusqlite::OptionalExtension;
use std::path::Path;
use std::path::PathBuf;

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

    pub fn update_track_bpm(&mut self, path: &Path, bpm: f32) -> rusqlite::Result<bool> {
        let updated = self.conn.execute(
            "UPDATE tracks
                 SET bpm = ?1
                 WHERE path = ?2",
            rusqlite::params![bpm, path.to_string_lossy().as_ref()],
        )?;

        Ok(updated > 0)
    }

    pub fn update_track_key(&mut self, path: &Path, key: &str) -> rusqlite::Result<bool> {
        let updated = self.conn.execute(
            "UPDATE tracks
                 SET key = ?1
                 WHERE path = ?2",
            rusqlite::params![key, path.to_string_lossy().as_ref()],
        )?;

        Ok(updated > 0)
    }
    pub fn load_cached_analysis(
        &self,
        path: &Path,
    ) -> rusqlite::Result<Option<CachedTrackAnalysis>> {
        self.conn
            .query_row(
                "SELECT
                    t.waveform_cache_path,
                    a.data
                 FROM tracks t
                 LEFT JOIN track_analysis_metadata a ON a.path = t.path
                 WHERE t.path = ?1",
                [path.to_string_lossy().as_ref()],
                |row| {
                    Ok(CachedTrackAnalysis {
                        waveform_cache_path: row.get::<_, Option<String>>(0)?.map(PathBuf::from),
                        data: row.get(1)?,
                    })
                },
            )
            .optional()
    }

    pub fn save_cached_analysis(
        &mut self,
        path: &Path,
        waveform_cache_path: Option<&Path>,
        data: &[u8],
    ) -> rusqlite::Result<bool> {
        let path_string = path.to_string_lossy();
        let waveform_cache_path =
            waveform_cache_path.map(|value| value.to_string_lossy().into_owned());

        let transaction = self.conn.transaction()?;

        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tracks WHERE path = ?1)",
            [path_string.as_ref()],
            |row| row.get(0),
        )?;

        if !exists {
            return Ok(false);
        }

        transaction.execute(
            "INSERT INTO track_analysis_metadata (path, data)
             VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET data = excluded.data",
            rusqlite::params![path_string.as_ref(), data],
        )?;

        transaction.execute(
            "UPDATE tracks
             SET waveform_cache_path = COALESCE(
                 ?1,
                 waveform_cache_path
             )
             WHERE path = ?2",
            rusqlite::params![waveform_cache_path, path_string.as_ref(),],
        )?;

        transaction.commit()?;

        Ok(true)
    }

    pub fn clear_analysis_cache(&mut self) -> rusqlite::Result<()> {
        let transaction = self.conn.transaction()?;

        transaction.execute("DELETE FROM track_analysis_metadata", [])?;
        transaction.execute(
            "UPDATE tracks
             SET waveform_cache_path = NULL,
                 bpm = NULL,
                 key = NULL",
            [],
        )?;

        transaction.commit()
    }
}

/// A manually edited BPM wins over the value produced by a later analysis run.
pub(crate) fn should_apply_detected_bpm(config: &AppConfig, path: &Path) -> bool {
    let path = path.to_string_lossy();

    config
        .ui
        .manual_grids
        .get(path.as_ref())
        .and_then(|grid| grid.tempo)
        .is_none()
}

#[cfg(test)]
mod analysis_tests {
    use super::should_apply_detected_bpm;
    use crate::shared::store_types::{AppConfig, ManualGrid};
    use std::path::Path;

    #[test]
    fn detected_bpm_should_be_applied_without_a_manual_tempo() {
        let config = AppConfig::default();

        assert!(should_apply_detected_bpm(&config, Path::new("music/a.wav")));
    }

    #[test]
    fn manual_tempo_should_block_detected_bpm() {
        let mut config = AppConfig::default();
        config.ui.manual_grids.insert(
            "music/a.wav".to_string(),
            ManualGrid {
                tempo: Some(128.0),
                anchor: None,
            },
        );

        assert!(!should_apply_detected_bpm(
            &config,
            Path::new("music/a.wav")
        ));
        assert!(should_apply_detected_bpm(&config, Path::new("music/b.wav")));
    }

    #[test]
    fn anchor_only_override_should_not_block_detected_bpm() {
        let mut config = AppConfig::default();
        config.ui.manual_grids.insert(
            "music/a.wav".to_string(),
            ManualGrid {
                tempo: None,
                anchor: Some(12.5),
            },
        );

        assert!(should_apply_detected_bpm(&config, Path::new("music/a.wav")));
    }
}
