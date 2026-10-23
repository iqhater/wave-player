CREATE TABLE track_analysis_metadata (
    path TEXT PRIMARY KEY NOT NULL,
    data BLOB NOT NULL,
    FOREIGN KEY (path) REFERENCES tracks(path) ON DELETE CASCADE
) WITHOUT ROWID;
