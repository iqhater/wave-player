use super::db::DbStore;
use crate::shared::store_types::Track;
use crate::shared::store_types::TrackData;
use std::path::{Path, PathBuf};

// Favorites table operations
impl DbStore {
    /// Добавить трек в избранное.
    ///
    /// Если трека ещё нет в tracks, сначала создаётся минимальная запись.
    /// Повторно добавить уже избранный трек невозможно.
    pub fn add_favorite(
        &mut self,
        path: &Path,
        track_data: &TrackData,
        cover_cache_path: Option<&Path>,
    ) -> rusqlite::Result<i64> {
        // Здесь path используется только для создания или поиска самого трека.
        let track_id = self.add_track(path, track_data, cover_cache_path)?;

        // Проверяем избранное строго по track_id.
        if self.is_favorite(track_id)? {
            return Ok(track_id);
        }

        self.conn.execute(
            "INSERT INTO favorites (track_id)
                 VALUES (?1)",
            rusqlite::params![track_id],
        )?;

        Ok(track_id)
    }

    /// Удалить трек из избранного по его id.
    pub fn remove_favorite(&mut self, track_id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "DELETE FROM favorites
             WHERE track_id = ?1",
            rusqlite::params![track_id],
        )?;

        Ok(())
    }

    /// Проверить, находится ли трек в избранном по id.
    pub fn is_favorite(&self, track_id: i64) -> rusqlite::Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(
                     SELECT 1
                     FROM favorites
                     WHERE track_id = ?1
                 )",
            rusqlite::params![track_id],
            |row| row.get(0),
        )
    }

    /// Список избранных треков.
    pub fn favorites(&mut self) -> Vec<Track> {
        self.conn
            .prepare(
                "SELECT t.id,
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
                        t.waveform_cache_path
                 FROM tracks t
                 INNER JOIN favorites f ON f.track_id = t.id
                 ORDER BY f.created_at DESC",
            )
            .expect("prepare favorites")
            .query_map([], |row| {
                Ok(Track {
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
                    waveform_cache_path: row.get::<_, Option<String>>(12)?.map(PathBuf::from),

                    ..Track::default()
                })
            })
            .expect("query favorites")
            .filter_map(|result| match result {
                Ok(track) => Some(track),
                Err(error) => {
                    eprintln!("failed to read favorite track: {error}");
                    None
                }
            })
            .collect()
    }

    fn add_track(
        &mut self,
        path: &Path,
        track_data: &TrackData,
        cover_cache_path: Option<&Path>,
    ) -> rusqlite::Result<i64> {
        let path_str = path.to_string_lossy();
        let cover_cache_path = cover_cache_path.map(|value| value.to_string_lossy().into_owned());

        let file_size = std::fs::metadata(path)
            .ok()
            .and_then(|metadata| i64::try_from(metadata.len()).ok())
            .unwrap_or(0);

        let title = track_data.title.clone();
        let artist = track_data.artist.clone();
        let album = track_data.album.clone();
        let genre = track_data.genre.clone();
        let duration = track_data.duration;

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
            rusqlite::params![
                path_str.as_ref(),
                file_size,
                title,
                artist,
                album,
                genre,
                duration,
                cover_cache_path,
            ],
        )?;

        self.conn.query_row(
            "SELECT id
             FROM tracks
             WHERE path = ?1",
            rusqlite::params![path_str.as_ref()],
            |row| row.get(0),
        )
    }

    pub fn increment_play_count(&mut self, path: &Path) -> rusqlite::Result<Option<i64>> {
        let path = path.to_string_lossy();
        let transaction = self.conn.transaction()?;

        let updated = transaction.execute(
            "UPDATE tracks
                 SET play_count = play_count + 1
                 WHERE path = ?1",
            rusqlite::params![path.as_ref()],
        )?;

        if updated == 0 {
            transaction.commit()?;
            return Ok(None);
        }

        let (track_id, play_count): (i64, i64) = transaction.query_row(
            "SELECT id, play_count
                 FROM tracks
                 WHERE path = ?1",
            rusqlite::params![path.as_ref()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        transaction.execute(
            "INSERT INTO recent_tracks (track_id, played_at)
                 VALUES (?1, CURRENT_TIMESTAMP)
                 ON CONFLICT(track_id) DO UPDATE SET
                     played_at = CURRENT_TIMESTAMP",
            rusqlite::params![track_id],
        )?;

        transaction.commit()?;

        Ok(Some(play_count))
    }
}
