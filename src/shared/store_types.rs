use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Единый трек в библиотеке.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub duration_secs: f32,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    /// id обложки в кэше (или None)
    pub cover_id: Option<String>,
    pub play_count: u64,
    pub added_at: i64, // unix ts
}

/// Избранное.
#[derive(Clone, Debug, PartialEq)]
pub struct Favorites {
    pub id: i64,
    pub name: String,
    pub track_paths: Vec<PathBuf>,
}

/// Запись прослушивания (история).
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub track_path: PathBuf,
    pub listened_at: i64, // unix ts
}

// ===== Настройки приложения (JSON) =====
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    /// Размер и позиция окна при последнем закрытии.
    pub window: WindowConfig,
    /// Выбранная тема.
    pub theme: ThemePreference,
    /// Параметры аудио-движка.
    pub audio: AudioConfig,
    /// Поведение интерфейса (страница Settings).
    pub interface: InterfaceConfig,
}

// ===== Окно =====
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WindowConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub maximized: bool,
}

// ===== Тема =====
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ThemePreference {
    Light,
    Dark,
}
impl Default for ThemePreference {
    fn default() -> Self {
        ThemePreference::Dark
    }
}

// ===== Аудио =====
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Playback
    pub volume: f32,
    pub speed: f32,
    pub preserve_pitch: bool,
    pub repeat_track: bool,

    /// Filters
    pub low_pass: Option<f32>,
    pub high_pass: Option<f32>,

    /// Spatial
    pub mid_side: u8, //TODO: enum
    pub stereo_width: f32,
    pub pan: f32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            // Playback
            volume: 0.35,
            speed: 1.0,
            preserve_pitch: false,
            repeat_track: false,

            // Filters
            low_pass: None,
            high_pass: None,

            // Spatial
            mid_side: 0,
            stereo_width: 1.0,
            pan: 0.0,
        }
    }
}

// ===== Поведение интерфейса =====
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InterfaceConfig {
    pub timeline_cursor_follows_loop: bool,
    pub show_time_markers: bool,
    pub loop_repeat_region: bool,
    pub timeline_progress_view: bool,
}
