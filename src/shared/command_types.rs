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
}
