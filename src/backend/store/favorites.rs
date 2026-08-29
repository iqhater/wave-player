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
    pub fn add_favorite(&mut self, path: &Path, track_data: &TrackData) -> rusqlite::Result<i64> {
        // Здесь path используется только для создания или поиска самого трека.
        let track_id = self.add_track(path, track_data)?;

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
                        t.play_count
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

                    ..Track::default() // audio_fx_params: row.get(11)?,
                                       // waveform_cache_path: row.get::<_, Option<String>>(12)?.map(PathBuf::from),
                                       // created_at: row.get(13)?,
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

    fn add_track(&mut self, path: &Path, track_data: &TrackData) -> rusqlite::Result<i64> {
        let path_str = path.to_string_lossy();

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
            "INSERT OR IGNORE INTO tracks (
                 path,
                 file_size,
                 title,
                 artist,
                 album,
                 genre,
                 duration
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                path_str.as_ref(),
                file_size,
                title,
                artist,
                album,
                genre,
                duration
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
}
