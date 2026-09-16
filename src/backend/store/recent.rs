use super::db::DbStore;
use crate::shared::store_types::{RecentTrack, TrackData};
use rusqlite::params;
use std::path::{Path, PathBuf};

impl DbStore {
    /// Создаёт или обновляет информацию о треке и добавляет его в recent_tracks.
    pub fn record_recent_track(
        &mut self,
        path: &Path,
        track_data: &TrackData,
        cover_cache_path: Option<&Path>,
    ) -> rusqlite::Result<i64> {
        let path_string = path.to_string_lossy();
        let cover_cache_path = cover_cache_path.map(|value| value.to_string_lossy().into_owned());

        let file_size = std::fs::metadata(path)
            .ok()
            .and_then(|metadata| i64::try_from(metadata.len()).ok())
            .unwrap_or(0);

        self.conn.execute(
            "INSERT INTO tracks (
                path,
                file_size,
                title,
                artist,
                album,
                genre,
                duration,
                cover_cache_path
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                genre = excluded.genre,
                duration = excluded.duration,
                cover_cache_path = COALESCE(
                                excluded.cover_cache_path,
                                tracks.cover_cache_path
                            )",
            params![
                path_string.as_ref(),
                file_size,
                track_data.title,
                track_data.artist,
                track_data.album,
                track_data.genre,
                track_data.duration,
                cover_cache_path,
            ],
        )?;

        let track_id = self.conn.query_row(
            "SELECT id
             FROM tracks
             WHERE path = ?1",
            params![path_string.as_ref()],
            |row| row.get::<_, i64>(0),
        )?;

        self.conn.execute(
            "INSERT INTO recent_tracks (track_id, played_at)
             VALUES (?1, CURRENT_TIMESTAMP)
             ON CONFLICT(track_id) DO UPDATE SET
                 played_at = CURRENT_TIMESTAMP",
            params![track_id],
        )?;

        Ok(track_id)
    }

    /// Возвращает недавние треки от новых к старым.
    pub fn recent_tracks(&self) -> Vec<RecentTrack> {
        let mut statement = match self.conn.prepare(
            "SELECT
                t.id,
                t.path,
                t.file_size,
                t.title,
                t.artist,
                t.album,
                t.genre,
                t.duration,
                t.bpm,
                t.key,
                t.play_count,
                t.cover_cache_path,
                r.played_at
             FROM recent_tracks r
             INNER JOIN tracks t ON t.id = r.track_id
             ORDER BY r.played_at DESC, t.id DESC",
        ) {
            Ok(statement) => statement,
            Err(error) => {
                eprintln!("failed to prepare recent tracks query: {error}");
                return Vec::new();
            }
        };

        let rows = match statement.query_map([], |row| {
            Ok(RecentTrack {
                track: crate::shared::store_types::Track {
                    id: row.get(0)?,
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    file_size: row.get(2)?,
                    title: row.get(3)?,
                    artist: row.get(4)?,
                    album: row.get(5)?,
                    genre: row.get(6)?,
                    duration: row.get(7)?,
                    bpm: row.get(8)?,
                    key: row.get(9)?,
                    play_count: row.get(10)?,
                    cover_cache_path: row.get::<_, Option<String>>(11)?.map(PathBuf::from),

                    ..Default::default()
                },
                played_at: row.get(12)?,
            })
        }) {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("failed to query recent tracks: {error}");
                return Vec::new();
            }
        };

        rows.filter_map(|row| match row {
            Ok(track) => Some(track),
            Err(error) => {
                eprintln!("failed to read recent track: {error}");
                None
            }
        })
        .collect()
    }

    pub fn remove_recent_track(&mut self, track_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM recent_tracks
             WHERE track_id = ?1",
            params![track_id],
        )?;

        Ok(())
    }
}
