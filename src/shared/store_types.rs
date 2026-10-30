use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Единый трек в библиотеке.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Track {
    pub id: i64,
    pub path: PathBuf,
    pub file_size: i64,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub genre: Option<String>,
    /// Длительность в миллисекундах.
    pub duration: i64,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub play_count: i64,
    /// JSON-объект с параметрами аудиоэффектов.
    pub audio_fx_params: Option<String>,
    pub waveform_cache_path: Option<PathBuf>,
    pub cover_cache_path: Option<PathBuf>,
    /// Timestamp в формате SQLite TEXT, например `2026-08-26 12:34:56`.
    pub created_at: String,
}

// DTO
#[derive(Clone, Debug, PartialEq)]
pub struct TrackData {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub genre: Option<String>,
    /// Длительность в миллисекундах.
    pub duration: i64,

    /// Raw cover bytes extracted from metadata or loaded from cache.
    pub cover: Vec<u8>,
}

/// Трек с датой последнего прослушивания.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentTrack {
    pub track: Track,

    /// Timestamp SQLite в формате `YYYY-MM-DD HH:MM:SS`.
    pub played_at: String,
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
    pub ui: UiConfig,
}

// ===== Окно =====
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WindowConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub x: Option<i32>,
    pub y: Option<i32>,
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
#[serde(default)]
pub struct AudioConfig {
    /// Playback
    pub volume: f32,
    pub speed: f32,
    pub preserve_pitch: bool,
    pub repeat_track: bool,

    /// Gain
    pub preamp_db: f32,

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

            preamp_db: 3.5,

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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub timeline_cursor_follows_loop: bool,
    pub show_time_markers: bool,
    pub loop_repeat_region: bool,
    pub timeline_progress_view: bool,
    pub show_beat_grid: bool,
    pub beat_grid_snap: bool,
    /// Show the translucent hover preview cursor over the waveform.
    pub show_hover_cursor: bool,
    /// Collapsed state of the desktop sidebar.
    pub sidebar_collapsed: bool,
    /// Per-track manual BPM and grid anchor overrides, keyed by track path.
    pub manual_grids: HashMap<String, ManualGrid>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            timeline_cursor_follows_loop: false,
            show_time_markers: true,
            loop_repeat_region: true,
            timeline_progress_view: false,
            show_beat_grid: true,
            beat_grid_snap: true,
            show_hover_cursor: true,
            sidebar_collapsed: false,
            manual_grids: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ManualGrid {
    pub tempo: Option<f32>,
    pub anchor: Option<f32>,
}

#[cfg(test)]
mod config_tests {
    use super::{AppConfig, ManualGrid, WindowConfig};

    #[test]
    fn config_should_roundtrip_all_settings() {
        let mut config = AppConfig::default();

        config.window = WindowConfig {
            width: Some(1600),
            height: Some(900),
            x: Some(120),
            y: Some(80),
            maximized: true,
        };
        config.audio.volume = 0.8;
        config.audio.speed = 1.25;
        config.audio.preserve_pitch = true;
        config.audio.repeat_track = true;
        config.audio.preamp_db = 5.0;
        config.audio.low_pass = Some(12_000.0);
        config.audio.high_pass = Some(40.0);
        config.audio.mid_side = 2;
        config.audio.stereo_width = 1.4;
        config.audio.pan = -0.3;
        config.ui.timeline_cursor_follows_loop = true;
        config.ui.show_time_markers = false;
        config.ui.loop_repeat_region = true;
        config.ui.timeline_progress_view = true;
        config.ui.show_beat_grid = false;
        config.ui.beat_grid_snap = true;
        config.ui.show_hover_cursor = false;
        config.ui.sidebar_collapsed = true;
        config.ui.manual_grids.insert(
            "music/track.wav".to_string(),
            super::ManualGrid {
                tempo: Some(128.0),
                anchor: Some(12.5),
            },
        );

        let json = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, config);
    }

    #[test]
    fn config_should_use_defaults_for_missing_gain_and_grid_fields() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();

        assert_eq!(config.audio.preamp_db, 3.5);
        assert!(config.ui.manual_grids.is_empty());
        assert!(config.ui.show_hover_cursor);
        assert!(!config.ui.sidebar_collapsed);
    }

    #[test]
    fn legacy_config_without_gain_and_grid_fields_should_still_load() {
        let json = r#"
        {
          "window": {"width": 1320, "height": 1002, "x": null, "y": null, "maximized": false},
          "theme": "Dark",
          "audio": {
            "volume": 0.5, "speed": 1.25, "preserve_pitch": true, "repeat_track": false,
            "low_pass": null, "high_pass": null,
            "mid_side": 0, "stereo_width": 1.0, "pan": 0.0
          },
          "ui": {
            "timeline_cursor_follows_loop": false, "show_time_markers": true,
            "loop_repeat_region": true, "timeline_progress_view": false,
            "show_beat_grid": true, "beat_grid_snap": false
          }
        }
        "#;

        let config: AppConfig = serde_json::from_str(json).unwrap();

        assert_eq!(config.audio.volume, 0.5);
        assert_eq!(config.audio.speed, 1.25);
        assert_eq!(config.audio.preamp_db, 3.5);
        assert!(config.ui.manual_grids.is_empty());
        assert!(config.ui.show_hover_cursor);
        assert!(!config.ui.sidebar_collapsed);
    }

    #[test]
    fn manual_grid_should_roundtrip_tempo_and_anchor() {
        let grid = ManualGrid {
            tempo: Some(127.5),
            anchor: Some(12.25),
        };

        let json = serde_json::to_string(&grid).unwrap();
        let restored: ManualGrid = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, grid);
    }
}
