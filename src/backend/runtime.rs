// TODO: engine state
use crate::shared::event_types::{AudioError, ErrorSender, StatusSender};
use rodio::Player;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant};

pub(crate) struct AudioRuntime {
    pub(crate) player: rodio::Player,

    pub(crate) track: TrackRuntime,
    pub(crate) processing: ProcessingRuntime,
    pub(crate) loop_state: LoopState,

    pub(crate) playback_progress: Arc<AtomicU32>,
    pub(crate) status_tx: StatusSender,
    pub(crate) error_tx: ErrorSender,
}

impl AudioRuntime {
    pub(crate) fn new(
        player: rodio::Player,
        playback_progress: Arc<AtomicU32>,
        status_tx: StatusSender,
        error_tx: ErrorSender,
    ) -> Self {
        Self {
            player,

            track: TrackRuntime {
                path: None,
                duration: Duration::ZERO,
                position: Duration::ZERO,
                is_loaded: false,
                is_playing: false,
                started_at: None,
                repeat: false,
            },

            processing: ProcessingRuntime {
                speed: 1.0,
                preserve_pitch: false,
                volume: 0.35,

                low_pass: Arc::new(AtomicU32::new(0)),
                high_pass: Arc::new(AtomicU32::new(0)),
                mid_side: Arc::new(AtomicU32::new(0)),
                stereo_width: Arc::new(AtomicU32::new(1.0f32.to_bits())),
                pan: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            },

            loop_state: LoopState {
                enabled: false,
                start: Duration::ZERO,
                end: Duration::ZERO,
            },

            playback_progress,
            status_tx,
            error_tx,
        }
    }
}

/* impl AudioRuntime {
    pub(crate) fn commit_position(&mut self) {
        if self.track.is_playing {
            self.track.advance(self.processing.speed);
        }
    }
} */

pub(crate) struct TrackRuntime {
    pub path: Option<PathBuf>,
    pub duration: Duration,
    pub position: Duration,
    pub is_loaded: bool,
    pub is_playing: bool,
    pub started_at: Option<Instant>,
    pub repeat: bool,
}

/* impl TrackRuntime {
    pub fn reset(&mut self) {
        self.position = Duration::ZERO;
        self.started_at = None;
        self.is_playing = false;
    }

    pub fn advance(&mut self, speed: f32) {
        if let Some(started_at) = self.started_at {
            self.position +=
                started_at.elapsed().mul_f32(speed);

            self.started_at =
                Some(Instant::now());
        }
    }
} */

pub(crate) struct LoopState {
    pub enabled: bool,
    pub start: Duration,
    pub end: Duration,
}

pub(crate) struct ProcessingRuntime {
    // Playback
    pub speed: f32,
    pub preserve_pitch: bool,
    pub volume: f32,

    // Filters & Spatial
    pub low_pass: Arc<AtomicU32>,
    pub high_pass: Arc<AtomicU32>,
    pub mid_side: Arc<AtomicU32>,
    pub stereo_width: Arc<AtomicU32>,
    pub pan: Arc<AtomicU32>,
}
