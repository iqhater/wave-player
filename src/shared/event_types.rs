use crate::backend::metadata::TrackMetadata;
use crate::shared::store_types::{AppConfig, RecentTrack, Track};
use serde::Serialize;
use std::path::PathBuf;

use async_broadcast::Sender as BroadcastSender;

use std::time::Duration;

// --- UI  <---  Audio Engine Events (События) ---
#[derive(Clone, Copy, Debug, Serialize)]
pub struct WaveformEnvelope {
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct BeatGridData {
    pub offset: f32,
    pub beats: Vec<f32>,
    pub bars: Vec<f32>,
    pub downbeats: Vec<f32>,
}

/* #[derive(Clone, Copy, Debug, Serialize)]
pub struct WaveformBin {
    pub min: f32,
    pub max: f32,
    pub rms: f32,
} */

// 2. Среднечастотный канал (Глобальный статус)
#[derive(Clone, Debug)]
pub enum PlayerEvent {
    Started {
        duration: Duration,
        metadata: TrackMetadata,
    },
    Paused,
    Resumed,
    Finished,

    /* VolumeChanged {
        volume: f32,
    }, */
    TrackLoaded,

    // Loop audio
    LoopSet {
        start: Duration,
        end: Duration,
    },
    LoopEnabled {
        is_loop: bool,
    },

    // Export Loop. perhaps move into StoreEvent?
    ExportStarted,

    ExportProgress {
        progress: f32,
    },

    ExportFinished {
        path: PathBuf,
    },

    ExportFailed,

    // Hard computed proccess
    WaveformChunkReady {
        chunk: Vec<f32>,
        start_bin: usize,
        total_bins: usize,
    },

    /* WaveformOverviewChunkReady {
        chunk: Vec<WaveformBin>,
        start_bin: usize,
        total_bins: usize,
    }, */
    WaveformDetailReady {
        request_id: u64,
        start_frame: usize,
        end_frame: usize,
        total_frames: usize,
        sample_rate: u32,
        points: Vec<WaveformEnvelope>,
    },

    // BPM & Key detection result
    BpmKeyDetected {
        bpm: f32,
        beat_grid: BeatGridData,
        key_name: String,
        key_numerical: String,
    },

    // Get LUFS
    LoudnessMeasured {
        momentary_lufs: Option<f32>,
        shortterm_lufs: Option<f32>,
        integrated_lufs: Option<f32>,
    },

    // Player state for different screens
    PlayerState {
        path: Option<PathBuf>,
        is_playing: bool,
    },
}
pub type StatusSender = BroadcastSender<PlayerEvent>;
// pub type StatusReceiver = BroadcastReceiver<PlayerEvent>;

// 3. Низкочастотный канал (Редкие ошибки)
#[derive(Clone, Debug)]
pub enum AudioError {
    // 📁 Проблемы с файловой системой
    FileNotFound(String), // Файл удалили или переименовали: "путь/к/треку.mp3"
    PermissionDenied,     // Нет прав доступа (например, на Android забыли дать доступ к памяти)

    // 🎵 Проблемы с декодированием
    UnsupportedFormat, // Пытаются открыть условный .txt или защищенный DRM файл
    SeekFailed,        // Если перемотка для источника не поддерживается
    FileCorrupted,     // Файл поврежден, декодер застрял на битых байтах
    RenderAudioFailed, // Ошибка экспорта рендера петли

    // 🔌 Проблемы с железом (Аудио-карта/Наушники)
    DeviceInitializationFailed, // На ПК нет звуковой карты или она заблокирована другой программой
    DeviceLost,                 // Отключили аудио-выход во время игры (выдернули USB-колонки)
}
pub type ErrorSender = BroadcastSender<AudioError>;
// pub type ErrorReceiver = BroadcastReceiver<AudioError>;

// ——— Снапшоты от Store ———
#[derive(Clone, Debug)]
pub enum StoreEvent {
    ConfigLoaded {
        config: AppConfig,
    },
    FavoritesLoaded {
        tracks: Vec<Track>,
    },
    // Favorites changed event
    FavoritesChanged {
        track_id: i64,
        is_favorite: bool,
    },
    /// Анализ трека обновился.
    TrackAnalysisUpdated {
        path: PathBuf,
        bpm: f32,
        key: String,
    },
    /// Счётчик прослушиваний обновился.
    TrackPlaysCountUpdated {
        path: PathBuf,
        count: u64,
    },

    RecentTracksLoaded {
        tracks: Vec<RecentTrack>,
    },

    RecentTrackChanged {
        track_id: i64,
        is_present: bool,
    },
}

pub type StoreSender = BroadcastSender<StoreEvent>;
// pub type StoreReceiver = BroadcastReceiver<StoreEvent>;
