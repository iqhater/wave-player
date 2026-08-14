// For store SQLite track metadata, info, BPM, key, listen counter, playlists, history...
use crate::shared::store_types::{HistoryEntry, Track};
use std::path::{Path, PathBuf};

pub struct DbStore {
    conn: rusqlite::Connection,
}

impl DbStore {
    pub fn open(path: &Path) -> Self {
        let conn = rusqlite::Connection::open(path).expect("open db");

        /*
        -- Индексы для быстрого поиска и сортировки (создавать отдельно)
        CREATE INDEX IF NOT EXISTS idx_tracks_artist_title ON tracks(artist, title);
        CREATE INDEX IF NOT EXISTS idx_tracks_bpm ON tracks(bpm);
        CREATE INDEX IF NOT EXISTS idx_tracks_play_count ON tracks(play_count DESC);


        time markers
        CREATE TABLE IF NOT EXISTS track_markers (
            id          INTEGER PRIMARY KEY,
            track_id    INTEGER NOT NULL,
            start_time  REAL NOT NULL,                       -- Время начала метки в секундах (например, 14.5)
            end_time    REAL,                                -- Время окончания (может быть NULL, если это просто точка, а не регион)
            label       TEXT NOT NULL,                       -- Название ('Intro', 'Chorus', 'Drop', 'Verse 1')
            color       TEXT,                                -- HEX-код цвета для визуализации в плеере (например, '#FF5733')

            -- Ограничение: время начала не должно быть отрицательным
            CHECK (start_time >= 0),
            -- Ограничение: конец региона не может быть раньше его начала
            CHECK (end_time IS NULL OR end_time > start_time),

            -- Связь с таблицей треков (удаляем метки, если удален трек)
            FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE
        );
         */
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tracks (
                id            INTEGER PRIMARY KEY,                              -- Автоматический ID (быстрее, чем с AUTOINCREMENT)
                path          TEXT NOT NULL UNIQUE,                             -- Путь уникален, пустые значения запрещены
                title         TEXT NOT NULL DEFAULT 'Unknown Track',
                artist        TEXT DEFAULT 'Unknown Artist',
                duration_secs INTEGER DEFAULT 0,                                -- Изменено на INTEGER для простоты и скорости
                bpm           REAL,                                             -- Оставляем REAL, если важна точность (например, 124.5)
                key           TEXT,                                             -- Тональность (например, '4A' или 'Am')
                cover_id      TEXT,
                play_count    INTEGER NOT NULL DEFAULT 0,                       -- Поле не может быть NULL
                added_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))  -- Автоматическая дата добавления (Unix Timestamp)
            );
            CREATE TABLE IF NOT EXISTS favorites (
                track_id  INTEGER PRIMARY KEY,                                  -- один трек нельзя добавить дважды
                added_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),     -- когда добавлен
                FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );",
        )
        .expect("init db");
        Self { conn }
    }

    pub fn edit_track(&mut self, path: &Path, duration_secs: f32) {
        // TODO: INSERT OR REPLACE + UPDATE added_at/play_count
    }
    pub fn increment_play_count(&mut self, path: &Path) {
        // TODO: UPDATE tracks SET play_count = play_count + 1 WHERE path = ?
    }
    pub fn play_count(&mut self, path: &Path) -> Option<u64> {
        // TODO: SELECT play_count FROM tracks WHERE path = ?
        None
    }

    // Favorites table operations
    /// Добавить трек в избранное (по пути). Если трека ещё нет в tracks — игнорируем.
    pub fn add_favorite(&mut self, path: &Path) {
        let path_str = path.to_string_lossy();
        self.conn
            .execute(
                "INSERT OR IGNORE INTO favorites (track_id)
                 SELECT id FROM tracks WHERE path = ?1",
                rusqlite::params![path_str],
            )
            .expect("add favorite");
    }

    /// Убрать трек из избранного (по пути).
    pub fn remove_favorite(&mut self, path: &Path) {
        let path_str = path.to_string_lossy();
        self.conn
            .execute(
                "DELETE FROM favorites WHERE track_id IN (SELECT id FROM tracks WHERE path = ?1)",
                rusqlite::params![path_str],
            )
            .expect("remove favorite");
    }

    /// Проверить, в избранном ли трек.
    pub fn is_favorite(&mut self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        self.conn
            .query_row(
                "SELECT 1 FROM favorites WHERE track_id IN (SELECT id FROM tracks WHERE path = ?1) LIMIT 1",
                rusqlite::params![path_str],
                |_| Ok(()),
            )
            .is_ok()
    }

    /// Список избранных треков (полные записи из tracks).
    pub fn favorites(&mut self) -> Vec<Track> {
        self.conn
            .prepare(
                "SELECT t.path, t.title, t.artist, t.duration_secs, t.bpm, t.key,
                        t.cover_id, t.play_count, t.added_at
                 FROM tracks t
                 INNER JOIN favorites f ON f.track_id = t.id
                 ORDER BY f.added_at DESC",
            )
            .expect("prepare favorites")
            .query_map([], |row| {
                Ok(Track {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    duration_secs: row.get::<_, i64>(3)? as f32,
                    bpm: row.get(4)?,
                    key: row.get(5)?,
                    cover_id: row.get(6)?,
                    play_count: row.get::<_, i64>(7)? as u64,
                    added_at: row.get(8)?,
                })
            })
            .expect("query favorites")
            .filter_map(|r| r.ok())
            .collect()
    }
}
