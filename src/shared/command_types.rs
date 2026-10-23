use super::store_types::{AppConfig, TrackData};
use std::path::PathBuf;
use std::time::Duration;

// --- UI  --->  Audio Engine (Команды) ---
#[derive(Clone, Debug)]
pub enum PlayerCommand {
    Play(PathBuf),
    Pause,
    Resume,
    Seek(Duration),
    SetVolume(f32),
    SetSpeed(f32),
    /// Сохранять тональность при изменении скорости
    SetPreservePitch(bool),

    // Loop audio
    SetLoop(Duration, Duration),
    ToggleLoop(bool),

    /// Load file with path
    LoadFile(String),

    PlayQueue {
        tracks: Vec<PathBuf>,
        current: PathBuf,
    },

    NextTrack,
    PreviousTrack,
    SetShuffle(bool),

    /// Repeat a whole track
    ToggleRepeatTrack(bool),
    /// Экспортировать петлю в WAV-файл по пути
    ExportLoop(String),

    /// Low-Pass фильтр: `Some(cutoff_hz)` — включить, `None` — выключить.
    SetLowPassFilter(Option<f32>),
    /// High-Pass фильтр: `Some(cutoff_hz)` — включить, `None` — выключить.
    SetHighPassFilter(Option<f32>),

    /// Изоляция M/S: `0` = стерео, `1` = Mid, `2` = Side.
    SetMidSideMode(u8),
    /// Ширина стереобазы `0.0..2.0` (норма `1.0`).
    SetStereoWidth(f32),
    /// Панорама `-1.0..1.0` (центр `0.0`).
    SetPan(f32),

    RequestWaveformDetail {
        request_id: u64,
        start: Duration,
        end: Duration,
        points: usize,
    },

    RequestWaveformOverview,

    /// Get player state for different screens
    RequestState,
}

#[derive(Clone, Debug)]
pub struct CachedTrackAnalysis {
    pub waveform_cache_path: Option<PathBuf>,
    pub data: Option<Vec<u8>>,
}

/// UI → Store: команды на сохранение/чтение. Отдельный канал, не трогает движок.
#[derive(Clone, Debug)]
pub enum StoreCommand {
    /// Увеличить счётчик прослушиваний трека и записать в историю.
    SaveTrackPlaysCount {
        path: PathBuf,
    },
    /// Сохранить/обновить метаданные трека в библиотеке.
    UpdateTrackInfo {
        path: PathBuf,
        duration_secs: f32,
    },

    /// Обновить анализ трека (BPM, ключ).
    UpdateTrackAnalysis {
        path: PathBuf,
        bpm: f32,
        key: String,
    },

    /// Изменить настройку приложения.
    SetConfig(AppConfig),
    /// Запросить снапшот (при старте UI).
    RequestSnapshot,

    // Favorites
    AddToFavorites {
        path: PathBuf,
        track_data: TrackData,
    },
    RemoveFromFavorites {
        track_id: i64,
    },
    GetFavorites,

    /// Recent tracks
    /// Записать успешное открытие трека в историю.
    RecordRecentTrack {
        path: PathBuf,
        track_data: TrackData,
    },
    /// Получить список недавних треков.
    GetRecentTracks,
    /// Удалить трек из истории.
    RemoveRecentTrack {
        track_id: i64,
    },

    LoadCachedAnalysis {
        path: PathBuf,
        reply: flume::Sender<Result<Option<CachedTrackAnalysis>, String>>,
    },

    SaveCachedAnalysis {
        path: PathBuf,
        waveform_cache_path: Option<PathBuf>,
        data: Vec<u8>,
    },
}
