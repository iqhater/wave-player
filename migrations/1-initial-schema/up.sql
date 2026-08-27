PRAGMA foreign_keys = ON;

-- 1. Главная таблица медиатеки
CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,              -- Полный путь к файлу
    file_size INTEGER NOT NULL,
    title TEXT NOT NULL DEFAULT 'Unknown Track',
    artist TEXT NOT NULL DEFAULT 'Unknown Artist',
    album TEXT,
    genre TEXT,
    duration INTEGER NOT NULL CHECK (duration >= 0),          -- Длительность в миллисекундах
    bpm REAL CHECK (bpm IS NULL OR bpm > 0),                           -- Темп (NULL, если еще не рассчитан)
    key TEXT,                           -- Тональность
    play_count INTEGER NOT NULL DEFAULT 0 CHECK (play_count >= 0), -- Количество прослушиваний
    audio_fx_params TEXT CHECK (audio_fx_params IS NULL OR (json_valid(audio_fx_params) AND json_type(audio_fx_params) = 'object')),                    -- Настройки фильтров и простарнства, эффектов
    waveform_cache_path TEXT,           -- Относительный путь к бинарному .wfm файлу кэша волны waveforms/3f/3f8b7c1a.wfm
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

-- 2. История прослушиваний (recent tracks)
CREATE TABLE IF NOT EXISTS recent_tracks (
    track_id INTEGER PRIMARY KEY,
    played_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

-- 3. Избранное (favorites)
CREATE TABLE IF NOT EXISTS favorites (
    track_id INTEGER PRIMARY KEY,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

-- 4. Маркеры и регионы структуры трека
CREATE TABLE IF NOT EXISTS track_markers (
    id INTEGER PRIMARY KEY,
    track_id INTEGER NOT NULL,
    position_start INTEGER NOT NULL CHECK (position_start >= 0),
    position_end INTEGER CHECK (
        position_end IS NULL
        OR position_end >= position_start
    ),                  -- NULL, если это точка, а не регион
    label TEXT NOT NULL,                -- "Intro", "Verse", etc.
    color TEXT,                         -- HEX-код цвета, например "#FF5733"
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

-- 5. Плейлисты (на будущее)
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP)
);

-- 6. Связующая таблица для плейлистов
CREATE TABLE IF NOT EXISTS playlist_tracks (
    id INTEGER PRIMARY KEY,
    playlist_id INTEGER NOT NULL,
    track_id INTEGER NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),          -- Порядковый номер трека внутри плейлиста
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

-- Индексы для ускорения выборок
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);
CREATE INDEX IF NOT EXISTS idx_track_markers_position ON track_markers(track_id, position_start);
CREATE INDEX IF NOT EXISTS idx_recent_tracks_played_at ON recent_tracks(played_at DESC);
