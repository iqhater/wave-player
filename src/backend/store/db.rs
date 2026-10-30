use include_dir::{Dir, include_dir};
use rusqlite::Connection;
use rusqlite_migration::Migrations;
use std::path::Path;
use std::sync::LazyLock;

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/migrations");

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::from_directory(&MIGRATIONS_DIR).expect("invalid database migrations")
});

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),
}

pub struct DbStore {
    pub(crate) conn: Connection,
}

impl DbStore {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let mut conn = Connection::open(path)?;

        // Настройки относятся к каждому соединению отдельно.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        // Применяет все миграции до последней версии.
        // MIGRATIONS.to_latest(&mut conn)?;
        // Применяет все миграции до последней версии.
        if let Err(error) = MIGRATIONS.to_latest(&mut conn) {
            eprintln!("failed to apply database migrations:");
            eprintln!("{error:#?}");

            return Err(DbError::Migration(error));
        }

        Ok(Self { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        assert!(MIGRATIONS.validate().is_ok());
    }

    #[test]
    fn migrations_apply_to_empty_database() {
        let mut connection = Connection::open_in_memory().unwrap();

        MIGRATIONS.to_latest(&mut connection).unwrap();

        let tracks_exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM sqlite_master
                    WHERE type = 'table' AND name = 'tracks'
                )",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert!(tracks_exists);
    }

    #[test]
    fn analysis_should_store_waveform_reference_separately() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();

        let mut db = DbStore { conn };

        db.conn
            .execute(
                "INSERT INTO tracks (path, file_size, duration)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params!["music/track.wav", 100, 1000],
            )
            .unwrap();

        let track = std::path::Path::new("music/track.wav");
        let waveform = std::path::Path::new("wavefroms/ab/example.v1.wfm");
        let metadata = br#"{"bpm_key":null,"loudness":null}"#;

        assert!(
            db.save_cached_analysis(track, Some(waveform), metadata)
                .unwrap()
        );

        let stored = db.load_cached_analysis(track).unwrap().unwrap();

        assert_eq!(
            (stored.waveform_cache_path, stored.data),
            (Some(waveform.to_path_buf()), Some(metadata.to_vec()))
        );
    }

    #[test]
    fn clear_analysis_cache_should_reset_track_analysis() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();

        let mut db = DbStore { conn };

        db.conn
            .execute(
                "INSERT INTO tracks
                    (path, file_size, duration, bpm, key, waveform_cache_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    "music/track.wav",
                    100,
                    1000,
                    120.0,
                    "Am",
                    "wavefroms/ab/example.v1.wfm"
                ],
            )
            .unwrap();

        let track = std::path::Path::new("music/track.wav");
        let waveform = std::path::Path::new("wavefroms/ab/example.v1.wfm");

        assert!(
            db.save_cached_analysis(track, Some(waveform), b"{}")
                .unwrap()
        );

        db.clear_analysis_cache().unwrap();

        let stored = db.load_cached_analysis(track).unwrap().unwrap();
        let (bpm, key): (Option<f32>, Option<String>) = db
            .conn
            .query_row(
                "SELECT bpm, key FROM tracks WHERE path = ?1",
                ["music/track.wav"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(stored.waveform_cache_path, None);
        assert_eq!(stored.data, None);
        assert_eq!((bpm, key), (None, None));
    }

    #[test]
    fn update_track_bpm_should_store_the_value_for_an_existing_track() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();

        let mut db = DbStore { conn };

        db.conn
            .execute(
                "INSERT INTO tracks (path, file_size, duration)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params!["music/track.wav", 100, 1000],
            )
            .unwrap();

        let track = std::path::Path::new("music/track.wav");

        assert!(db.update_track_bpm(track, 128.0).unwrap());

        let bpm: Option<f32> = db
            .conn
            .query_row(
                "SELECT bpm FROM tracks WHERE path = ?1",
                ["music/track.wav"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(bpm, Some(128.0));
    }

    #[test]
    fn update_track_bpm_should_report_a_missing_track() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();

        let mut db = DbStore { conn };

        assert!(
            !db.update_track_bpm(std::path::Path::new("music/missing.wav"), 128.0)
                .unwrap()
        );
    }

    #[test]
    fn update_track_key_should_not_change_the_bpm() {
        let mut conn = Connection::open_in_memory().unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();

        let mut db = DbStore { conn };

        db.conn
            .execute(
                "INSERT INTO tracks (path, file_size, duration, bpm)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params!["music/track.wav", 100, 1000, 128.0],
            )
            .unwrap();

        let track = std::path::Path::new("music/track.wav");

        assert!(db.update_track_key(track, "Am").unwrap());

        let (bpm, key): (Option<f32>, Option<String>) = db
            .conn
            .query_row(
                "SELECT bpm, key FROM tracks WHERE path = ?1",
                ["music/track.wav"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(bpm, Some(128.0));
        assert_eq!(key.as_deref(), Some("Am"));
    }
}
