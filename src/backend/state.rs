use crate::backend::analysis::service::AnalysisService;
use crate::backend::meter::{MeterState, linear_to_dbfs};
use crate::backend::queue::QueueState;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{ErrorSender, PlayerEvent, StatusSender};

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
    pub(crate) queue: QueueState,

    pub(crate) status_tx: StatusSender,
    pub(crate) store_cmd_tx: flume::Sender<StoreCommand>,
    pub(crate) error_tx: ErrorSender,

    pub(crate) cover_cache_dir: PathBuf,
    /// Writable application data directory, used as the base for relative
    /// export paths.
    pub(crate) data_dir: PathBuf,

    pub(crate) meter: MeterState,
    last_meter_update: Instant,

    pub(crate) analysis: AnalysisService,
}

impl AudioState {
    pub(crate) fn new(
        player: Player,
        playback_progress: Arc<AtomicU32>,
        status_tx: StatusSender,
        store_cmd_tx: flume::Sender<StoreCommand>,
        error_tx: ErrorSender,
        cover_cache_dir: PathBuf,
        data_dir: PathBuf,
    ) -> Self {
        let analysis = AnalysisService::new(cover_cache_dir.clone(), store_cmd_tx.clone());

        Self {
            player,

            track: TrackState::new(),
            playback: PlaybackState::new(playback_progress),
            effects: EffectsState::new(),
            loop_state: LoopState::new(),
            queue: QueueState::default(),

            status_tx,
            store_cmd_tx,
            error_tx,
            cover_cache_dir,
            data_dir,

            meter: MeterState::new(),
            last_meter_update: Instant::now(),

            analysis,
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

    pub(crate) fn publish_queue_state(&self) {
        let _ = self.status_tx.try_broadcast(PlayerEvent::QueueChanged {
            length: self.queue.len(),
            has_previous: self.queue.has_previous(),
            has_next: self.queue.has_next(),
            shuffle: self.queue.shuffle_enabled(),
        });
    }

    pub(crate) fn publish_meter_if_needed(&mut self, is_playing: bool) {
        const UPDATE_INTERVAL: Duration = Duration::from_millis(50);

        if self.last_meter_update.elapsed() < UPDATE_INTERVAL {
            return;
        }

        let reading = if is_playing {
            self.meter.read()
        } else {
            self.meter.reset();

            crate::backend::meter::MeterReading {
                peak: 0.0,
                rms: 0.0,
            }
        };

        let _ = self
            .status_tx
            .try_broadcast(PlayerEvent::OutputLevelMeasured {
                peak_db: linear_to_dbfs(reading.peak),
                rms_db: linear_to_dbfs(reading.rms),
            });

        self.last_meter_update = Instant::now();
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
    pub finished: bool,
    pub session_listened_time: Duration,
    pub play_count_recorded: bool,
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
            finished: false,
            session_listened_time: Duration::ZERO,
            play_count_recorded: false,
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

    pub preamp_db: f32,
    pub gain: Arc<AtomicU32>,
}

impl PlaybackState {
    fn new(playback_progress: Arc<AtomicU32>) -> Self {
        let preamp_db = 3.5;
        let gain = 10.0_f32.powf(preamp_db / 20.0);

        Self {
            playback_progress,
            volume: 0.35,
            speed: 1.0,
            speed_shared: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            preserve_pitch: false,
            preserve_pitch_shared: Arc::new(AtomicBool::new(false)),
            last_progress_update: Instant::now(),

            preamp_db,
            gain: Arc::new(AtomicU32::new(gain.to_bits())),
        }
    }

    pub(crate) fn gain_db(&self) -> f32 {
        self.preamp_db.clamp(-36.0, 24.0)
    }

    pub(crate) fn apply_gain(&self) {
        let gain = 10.0_f32.powf(self.gain_db() / 20.0);

        self.gain.store(gain.to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn set_preamp_db(&mut self, db: f32) {
        if db.is_finite() {
            self.preamp_db = db.clamp(-24.0, 24.0);
            self.apply_gain();
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

#[cfg(test)]
mod gain_tests {
    use super::PlaybackState;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    fn state() -> PlaybackState {
        PlaybackState::new(Arc::new(AtomicU32::new(0)))
    }

    #[test]
    fn gain_db_should_follow_preamp() {
        let mut state = state();
        state.set_preamp_db(-6.0);

        assert_eq!(state.gain_db(), -6.0);
    }

    #[test]
    fn preamp_should_be_clamped_to_ui_range() {
        let mut state = state();

        state.set_preamp_db(100.0);
        assert_eq!(state.preamp_db, 24.0);

        state.set_preamp_db(-100.0);
        assert_eq!(state.preamp_db, -24.0);
    }

    #[test]
    fn invalid_preamp_should_be_ignored() {
        let mut state = state();
        state.set_preamp_db(1.0);
        state.set_preamp_db(f32::NAN);

        assert_eq!(state.preamp_db, 1.0);
    }
}
