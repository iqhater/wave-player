use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{ErrorSender, StatusSender};

use rodio::Player;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

// Audio global state
pub(crate) struct AudioState {
    pub(crate) player: Player,

    pub(crate) track: TrackState,
    pub(crate) playback: PlaybackState,
    pub(crate) effects: EffectsState,
    pub(crate) loop_state: LoopState,

    pub(crate) status_tx: StatusSender,
    pub(crate) store_cmd_tx: flume::Sender<StoreCommand>,
    pub(crate) error_tx: ErrorSender,
}

impl AudioState {
    pub(crate) fn new(
        player: Player,
        playback_progress: Arc<AtomicU32>,
        status_tx: StatusSender,
        store_cmd_tx: flume::Sender<StoreCommand>,
        error_tx: ErrorSender,
    ) -> Self {
        Self {
            player,

            track: TrackState::new(),
            playback: PlaybackState::new(playback_progress),
            effects: EffectsState::new(),
            loop_state: LoopState::new(),

            status_tx,
            store_cmd_tx,
            error_tx,
        }
    }
    pub(crate) fn set_progress(&self, position: Duration) {
        self.playback
            .playback_progress
            .store(position.as_millis() as u32, Ordering::Relaxed);
    }

    pub(crate) fn publish_progress_if_needed(&mut self) {
        if self.playback.last_progress_update.elapsed() >= Duration::from_millis(50) {
            self.set_progress(self.track.position);
            self.playback.last_progress_update = Instant::now();
        }
    }
}

// Track control
pub(crate) struct TrackState {
    pub path: Option<PathBuf>,
    pub duration: Duration,
    pub position: Duration,
    pub is_loaded: bool,
    pub is_playing: bool,
    pub started_at: Option<Instant>,
    pub repeat: bool,
}

impl TrackState {
    fn new() -> Self {
        Self {
            path: None,
            duration: Duration::ZERO,
            position: Duration::ZERO,
            is_loaded: false,
            is_playing: false,
            started_at: None,
            repeat: false,
        }
    }
}

// Loop region
pub(crate) struct LoopState {
    pub enabled: bool,
    pub start: Duration,
    pub end: Duration,
}

impl LoopState {
    fn new() -> Self {
        Self {
            enabled: false,
            start: Duration::ZERO,
            end: Duration::ZERO,
        }
    }
}

// Playback
pub(crate) struct PlaybackState {
    pub playback_progress: Arc<AtomicU32>,
    pub volume: f32,
    pub speed: f32,
    pub speed_shared: Arc<AtomicU32>,
    pub preserve_pitch: bool,
    pub preserve_pitch_shared: Arc<AtomicBool>,
    pub last_progress_update: Instant,
}

impl PlaybackState {
    fn new(playback_progress: Arc<AtomicU32>) -> Self {
        Self {
            playback_progress,
            volume: 0.35,
            speed: 1.0,
            speed_shared: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            preserve_pitch: false,
            preserve_pitch_shared: Arc::new(AtomicBool::new(false)),
            last_progress_update: Instant::now(),
        }
    }
}

// Filters & Spatial
pub(crate) struct EffectsState {
    pub low_pass: Arc<AtomicU32>,
    pub high_pass: Arc<AtomicU32>,
    pub mid_side: Arc<AtomicU32>,
    pub stereo_width: Arc<AtomicU32>,
    pub pan: Arc<AtomicU32>,
}

impl EffectsState {
    fn new() -> Self {
        Self {
            low_pass: Arc::new(AtomicU32::new(0)),
            high_pass: Arc::new(AtomicU32::new(0)),
            mid_side: Arc::new(AtomicU32::new(0)),
            stereo_width: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            pan: Arc::new(AtomicU32::new(0.0f32.to_bits())),
        }
    }
}
