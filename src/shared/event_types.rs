use crate::backend::metadata::TrackMetadata;
use async_broadcast::Sender as BroadcastSender;
use std::time::Duration;

// --- UI  <---  Audio Engine Events (События) ---

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

    // VolumeChanged, ??
    TrackLoaded,

    // Loop audio
    LoopSet {
        start: Duration,
        end: Duration,
    },
    LoopEnabled {
        is_loop: bool,
    },

    // Hard computed proccess
    WaveformChunkReady {
        chunk: Vec<f32>,
        start_bin: usize,
        total_bins: usize,
    },

    // BPM & Key detection result
    BpmKeyDetected {
        bpm: f32,
        key_name: String,
        key_numerical: String,
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

    // 🔌 Проблемы с железом (Аудио-карта/Наушники)
    DeviceInitializationFailed, // На ПК нет звуковой карты или она заблокирована другой программой
    DeviceLost,                 // Отключили аудио-выход во время игры (выдернули USB-колонки)
}
pub type ErrorSender = BroadcastSender<AudioError>;
// pub type ErrorReceiver = BroadcastReceiver<AudioError>;
