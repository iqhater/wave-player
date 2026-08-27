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
}
