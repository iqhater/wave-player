use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use ebur128::{EbuR128, Mode};
use rodio::Source;
use serde::{Deserialize, Serialize};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::bpm_key::{BPM_KEY_VERSION, BpmKeyResult, analyze_bpm_key};
use crate::backend::decoder::{SeekAccuracy, open_file_with_decoder};
use crate::backend::waveform_cache::{self, CachedWaveform, CachedWaveformPeaks, SourceStamp};
use crate::shared::command_types::CachedTrackAnalysis;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{PlayerEvent, StatusSender, WaveformEnvelope};

#[cfg(test)]
use crate::backend::waveform_cache::CACHE_VERSION;

pub(crate) const ANALYSIS_DIR: &str = "analysis";
const OVERVIEW_BINS: usize = 500;
const MAX_DETAIL_POINTS: usize = 16_384;
const CHECK_INTERVAL: usize = 4096;

pub(crate) const PEAK_BIN_FRAMES: usize = 128;
pub(crate) const MAX_PEAK_BINS: usize = 4_000_000;

/// Finest level of the waveform peak pyramid built during track analysis.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WaveformPeaks {
    pub(crate) sample_rate: u32,
    pub(crate) total_frames: usize,
    pub(crate) bin_frames: usize,
    /// Packed `min`/`max` pairs for each bin.
    pub(crate) bins: Vec<i8>,
}

type AnalysisResult<T> = Result<T, Failure>;

#[derive(Debug)]
enum Failure {
    Cancelled,
    Failed(String),
}

fn failed(error: impl std::fmt::Debug) -> Failure {
    Failure::Failed(format!("{error:?}"))
}

#[derive(Clone)]
struct Token {
    track_epoch: Arc<AtomicU64>,
    track_generation: u64,
    detail: Option<(Arc<AtomicU64>, u64)>,
}

impl Token {
    fn cancelled(&self) -> bool {
        self.track_epoch.load(Ordering::Acquire) != self.track_generation
            || self
                .detail
                .as_ref()
                .is_some_and(|(epoch, generation)| epoch.load(Ordering::Acquire) != *generation)
    }

    fn check(&self) -> AnalysisResult<()> {
        if self.cancelled() {
            Err(Failure::Cancelled)
        } else {
            Ok(())
        }
    }
}

struct Latest<T> {
    tx: flume::Sender<T>,
    pending_rx: flume::Receiver<T>,
}

impl<T: Send + 'static> Latest<T> {
    fn new(run: impl Fn(T) + Send + 'static) -> Self {
        let (tx, rx) = flume::bounded(1);
        let pending_rx = rx.clone();

        thread::spawn(move || {
            if let Err(error) =
                thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Min)
            {
                eprintln!("failed to lower analysis thread priority: {error}");
            }

            while let Ok(job) = rx.recv() {
                run(job);
            }
        });

        Self { tx, pending_rx }
    }

    fn submit(&self, mut job: T) {
        loop {
            match self.tx.try_send(job) {
                Ok(()) => return,
                Err(flume::TrySendError::Full(value)) => {
                    job = value;

                    // Only pending work is replaced; running work owns its token.
                    let _ = self.pending_rx.try_recv();
                }
                Err(flume::TrySendError::Disconnected(_)) => return,
            }
        }
    }
}

struct TrackJob {
    path: PathBuf,
    duration: Duration,
    token: Token,
    force: bool,
}

struct DetailJob {
    path: PathBuf,
    duration: Duration,
    request_id: u64,
    start: Duration,
    end: Duration,
    points: usize,
    token: Token,
}

struct Output {
    token: Token,
    event: PlayerEvent,
}

pub(crate) struct AnalysisService {
    track_epoch: Arc<AtomicU64>,
    detail_epoch: Arc<AtomicU64>,
    tracks: Latest<TrackJob>,
    details: Latest<DetailJob>,
    output_rx: flume::Receiver<Output>,
    overview: Option<Vec<f32>>,
    waveform_peaks: Option<PlayerEvent>,
}

impl AnalysisService {
    pub(crate) fn new(cache_dir: PathBuf, store_tx: flume::Sender<StoreCommand>) -> Self {
        let track_epoch = Arc::new(AtomicU64::new(0));
        let detail_epoch = Arc::new(AtomicU64::new(0));
        let (output_tx, output_rx) = flume::bounded(8);

        let track_output = output_tx.clone();

        let tracks = Latest::new(move |job: TrackJob| {
            // Avoid starting expensive work while the user is still browsing.
            thread::sleep(Duration::from_millis(150));

            if let Err(Failure::Failed(message)) =
                analyze_track(&job, &cache_dir, &store_tx, &track_output)
            {
                eprintln!(
                    "track analysis failed for {}: {message}",
                    job.path.display()
                );
            }
        });

        let details = Latest::new(move |job: DetailJob| {
            if let Err(Failure::Failed(message)) = analyze_detail(&job, &output_tx) {
                eprintln!(
                    "waveform detail failed for {}: {message}",
                    job.path.display()
                );
            }
        });

        Self {
            track_epoch,
            detail_epoch,
            tracks,
            details,
            output_rx,
            overview: None,
            waveform_peaks: None,
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.track_epoch.fetch_add(1, Ordering::AcqRel);
        self.detail_epoch.fetch_add(1, Ordering::AcqRel);
        self.overview = None;
        self.waveform_peaks = None;
    }

    fn track_token(&self) -> Token {
        Token {
            track_epoch: self.track_epoch.clone(),
            track_generation: self.track_epoch.load(Ordering::Acquire),
            detail: None,
        }
    }

    pub(crate) fn start(&self, path: PathBuf, duration: Duration) {
        self.tracks.submit(TrackJob {
            path,
            duration,
            token: self.track_token(),
            force: false,
        });
    }

    /// Discards cached results and re-runs the full analysis pipeline.
    pub(crate) fn restart(&mut self, path: PathBuf, duration: Duration) {
        self.cancel();

        self.tracks.submit(TrackJob {
            path,
            duration,
            token: self.track_token(),
            force: true,
        });
    }

    pub(crate) fn request_detail(
        &self,
        path: PathBuf,
        duration: Duration,
        request_id: u64,
        start: Duration,
        end: Duration,
        points: usize,
    ) {
        let generation = self.detail_epoch.fetch_add(1, Ordering::AcqRel) + 1;

        let mut token = self.track_token();
        token.detail = Some((self.detail_epoch.clone(), generation));

        self.details.submit(DetailJob {
            path,
            duration,
            request_id,
            start,
            end,
            points,
            token,
        });
    }

    /// Publishes pending analysis events to the UI.
    pub(crate) fn publish(&mut self, status_tx: &StatusSender) {
        while let Ok(output) = self.output_rx.try_recv() {
            if output.token.cancelled() {
                continue;
            }

            match &output.event {
                PlayerEvent::WaveformChunkReady { chunk, .. } => {
                    self.overview = Some(chunk.clone());
                }
                PlayerEvent::WaveformPeaksReady { .. } => {
                    // Keep the payload for replay after the component remounts.
                    self.waveform_peaks = Some(output.event.clone());
                }
                _ => {}
            }

            // Publication and track switching run on the same engine thread.
            let _ = status_tx.try_broadcast(output.event);
        }
    }

    pub(crate) fn replay_overview(&self, status_tx: &StatusSender) {
        if let Some(peaks) = &self.overview {
            let _ = status_tx.try_broadcast(overview_event(peaks));
        }

        if let Some(peaks) = &self.waveform_peaks {
            let _ = status_tx.try_broadcast(peaks.clone());
        }
    }
}

impl Drop for AnalysisService {
    fn drop(&mut self) {
        // Do not join a possibly running synchronous DSP call on shutdown.
        self.cancel();
    }
}

fn emit(tx: &flume::Sender<Output>, token: &Token, event: PlayerEvent) -> AnalysisResult<()> {
    let mut output = Output {
        token: token.clone(),
        event,
    };

    loop {
        token.check()?;

        match tx.send_timeout(output, Duration::from_millis(25)) {
            Ok(()) => return Ok(()),
            Err(flume::SendTimeoutError::Timeout(value)) => {
                output = value;
            }
            Err(flume::SendTimeoutError::Disconnected(_)) => {
                return Err(Failure::Cancelled);
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct LoudnessResult {
    integrated_lufs: Option<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CachedAnalysis {
    source: SourceStamp,

    // Waveform samples never enter the metadata JSON or database.
    #[serde(skip)]
    waveform: Option<Vec<f32>>,

    // The finest peak pyramid follows the waveform cache file.
    #[serde(skip)]
    waveform_peaks: Option<WaveformPeaks>,

    // The persistent reference belongs to tracks.waveform_cache_path.
    #[serde(skip)]
    waveform_cache_path: Option<PathBuf>,

    bpm_key: Option<BpmKeyResult>,
    #[serde(default)]
    bpm_key_version: u32,
    loudness: Option<LoudnessResult>,
}

impl CachedAnalysis {
    fn empty(source: SourceStamp) -> Self {
        Self {
            source,
            waveform: None,
            waveform_peaks: None,
            waveform_cache_path: None,
            bpm_key: None,
            bpm_key_version: BPM_KEY_VERSION,
            loudness: None,
        }
    }

    fn complete(&self) -> bool {
        self.waveform.is_some()
            && self.waveform_peaks.is_some()
            && self.bpm_key.is_some()
            && self.loudness.is_some()
    }

    fn merge(&mut self, other: Self) {
        if self.source != other.source {
            return;
        }

        if self.bpm_key.is_none() {
            self.bpm_key = other.bpm_key;
        }

        if self.loudness.is_none() {
            self.loudness = other.loudness;
        }
    }
}

fn persist(
    path: &Path,
    cache_dir: &Path,
    store_tx: &flume::Sender<StoreCommand>,
    cache: &mut CachedAnalysis,
) -> AnalysisResult<()> {
    if SourceStamp::read(path).map_err(failed)? != cache.source {
        return Err(Failure::Cancelled);
    }

    if cache.waveform_cache_path.is_none()
        && let (Some(peaks), Some(waveform_peaks)) =
            (cache.waveform.as_ref(), cache.waveform_peaks.as_ref())
    {
        let waveform = CachedWaveform {
            peaks: peaks.clone(),
            waveform_peaks: Some(cached_waveform_peaks(waveform_peaks)),
        };

        match waveform_cache::save(cache_dir, &cache.source, &waveform) {
            Ok(relative) => {
                cache.waveform_cache_path = Some(relative);
            }
            Err(error) => {
                eprintln!("failed to save waveform for {}: {error}", path.display());
            }
        }
    }

    let bytes = serde_json::to_vec(cache).map_err(failed)?;

    // This JSON contains metadata only, not waveform samples.
    let file = cache_path(cache_dir, path);
    let temporary = file.with_extension(format!("{}.tmp", std::process::id()));

    let write_result = (|| -> std::io::Result<()> {
        fs::create_dir_all(cache_dir.join(ANALYSIS_DIR))?;
        fs::write(&temporary, &bytes)?;
        fs::rename(&temporary, &file)
    })();

    if let Err(error) = write_result {
        eprintln!(
            "failed to write analysis metadata cache for {}: {error}",
            path.display()
        );

        let _ = fs::remove_file(&temporary);
    }

    if let Err(error) = store_tx.send_timeout(
        StoreCommand::SaveCachedAnalysis {
            path: path.to_path_buf(),
            waveform_cache_path: cache.waveform_cache_path.clone(),
            data: bytes,
        },
        Duration::from_secs(2),
    ) {
        eprintln!(
            "failed to queue analysis metadata for {}: {error}",
            path.display()
        );
    }

    if let Some(result) = &cache.bpm_key {
        if let Err(error) = store_tx.send_timeout(
            StoreCommand::UpdateTrackAnalysis {
                path: path.to_path_buf(),
                bpm: result.bpm,
                key: result.key_name.clone(),
            },
            Duration::from_secs(2),
        ) {
            eprintln!(
                "failed to queue BPM/key update for {}: {error}",
                path.display()
            );
        }
    }

    Ok(())
}

fn load_cache(
    job: &TrackJob,
    cache_dir: &Path,
    store_tx: &flume::Sender<StoreCommand>,
) -> AnalysisResult<CachedAnalysis> {
    job.token.check()?;

    let source = SourceStamp::read(&job.path).map_err(failed)?;

    let mut cache = fs::read(cache_path(cache_dir, &job.path))
        .ok()
        .and_then(|bytes| parse_cache(&bytes, &source))
        .unwrap_or_else(|| CachedAnalysis::empty(source));

    let stored = request_stored_analysis(job, store_tx)?;

    let database_path = stored
        .as_ref()
        .and_then(|value| value.waveform_cache_path.clone());

    if let Some(data) = stored.as_ref().and_then(|value| value.data.as_deref()) {
        if let Some(metadata) = parse_cache(data, &cache.source) {
            cache.merge(metadata);
        }
    }

    let expected_path = waveform_cache::relative_path(&cache.source).map_err(failed)?;

    let mut candidates = Vec::with_capacity(2);

    if let Some(path) = &database_path {
        candidates.push(path.clone());
    }

    // Recover a completed file if the application stopped before its
    // database reference was saved.
    if !candidates.contains(&expected_path) {
        candidates.push(expected_path);
    }

    for relative in candidates {
        job.token.check()?;

        match waveform_cache::load(cache_dir, &relative, &cache.source, OVERVIEW_BINS) {
            Ok(Some(cached)) => {
                cache.waveform = Some(cached.peaks);
                cache.waveform_peaks = cached.waveform_peaks.map(waveform_peaks_from_cache);
                cache.waveform_cache_path = Some(relative);
                break;
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!("cannot use waveform cache {}: {error}", relative.display());
            }
        }
    }

    let recovered_reference =
        cache.waveform_cache_path.is_some() && cache.waveform_cache_path != database_path;

    if recovered_reference {
        persist(&job.path, cache_dir, store_tx, &mut cache)?;
    }

    Ok(cache)
}

fn parse_cache(bytes: &[u8], source: &SourceStamp) -> Option<CachedAnalysis> {
    let mut cache = serde_json::from_slice::<CachedAnalysis>(bytes)
        .ok()
        .filter(|cache| &cache.source == source)?;

    // Reanalyze grids made on a trimmed timeline without discarding other stages.
    if cache.bpm_key_version != BPM_KEY_VERSION {
        cache.bpm_key = None;
        cache.bpm_key_version = BPM_KEY_VERSION;
    }

    Some(cache)
}

fn cache_path(cache_dir: &Path, path: &Path) -> PathBuf {
    let mut hash = 0xcbf29ce484222325_u64;

    for byte in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    cache_dir
        .join(ANALYSIS_DIR)
        .join(format!("{hash:016x}.json"))
}

fn request_stored_analysis(
    job: &TrackJob,
    store_tx: &flume::Sender<StoreCommand>,
) -> AnalysisResult<Option<CachedTrackAnalysis>> {
    job.token.check()?;

    let (reply, response) = flume::bounded(1);

    store_tx
        .send_timeout(
            StoreCommand::LoadCachedAnalysis {
                path: job.path.clone(),
                reply,
            },
            Duration::from_millis(100),
        )
        .map_err(|error| Failure::Failed(format!("cannot request cached analysis: {error}")))?;

    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        job.token.check()?;

        match response.recv_timeout(Duration::from_millis(25)) {
            Ok(result) => return result.map_err(Failure::Failed),
            Err(flume::RecvTimeoutError::Disconnected) => {
                return Err(Failure::Failed(
                    "analysis metadata reply channel disconnected".into(),
                ));
            }
            Err(flume::RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    return Err(Failure::Failed("analysis metadata lookup timed out".into()));
                }
            }
        }
    }
}

fn overview_event(peaks: &[f32]) -> PlayerEvent {
    PlayerEvent::WaveformChunkReady {
        start_bin: 0,
        total_bins: peaks.len(),
        chunk: peaks.to_vec(),
    }
}

fn cached_waveform_peaks(peaks: &WaveformPeaks) -> CachedWaveformPeaks {
    CachedWaveformPeaks {
        sample_rate: peaks.sample_rate,
        total_frames: peaks.total_frames as u64,
        bin_frames: peaks.bin_frames as u32,
        bins: peaks.bins.clone(),
    }
}

fn waveform_peaks_from_cache(peaks: CachedWaveformPeaks) -> WaveformPeaks {
    WaveformPeaks {
        sample_rate: peaks.sample_rate,
        total_frames: peaks.total_frames as usize,
        bin_frames: peaks.bin_frames as usize,
        bins: peaks.bins,
    }
}

fn waveform_peaks_event(peaks: &WaveformPeaks) -> PlayerEvent {
    let bytes: Vec<u8> = peaks.bins.iter().map(|bin| *bin as u8).collect();
    let encoded: Arc<str> = STANDARD.encode(bytes).into();

    PlayerEvent::WaveformPeaksReady {
        sample_rate: peaks.sample_rate,
        total_frames: peaks.total_frames,
        bin_frames: peaks.bin_frames,
        encoded,
    }
}

fn bpm_event(result: &BpmKeyResult) -> PlayerEvent {
    PlayerEvent::BpmKeyDetected {
        bpm: result.bpm,
        beat_grid: result.beat_grid.clone(),
        key_name: result.key_name.clone(),
        key_numerical: result.key_numerical.clone(),
    }
}

fn loudness_event(result: &LoudnessResult) -> PlayerEvent {
    PlayerEvent::LoudnessMeasured {
        momentary_lufs: None,
        shortterm_lufs: None,
        integrated_lufs: result.integrated_lufs,
    }
}

fn analyze_track(
    job: &TrackJob,
    cache_dir: &Path,
    store_tx: &flume::Sender<StoreCommand>,
    output_tx: &flume::Sender<Output>,
) -> AnalysisResult<()> {
    let mut cache = if job.force {
        CachedAnalysis::empty(SourceStamp::read(&job.path).map_err(failed)?)
    } else {
        load_cache(job, cache_dir, store_tx)?
    };

    if let Some(peaks) = &cache.waveform {
        emit(output_tx, &job.token, overview_event(peaks))?;
    }

    if let Some(peaks) = &cache.waveform_peaks {
        emit(output_tx, &job.token, waveform_peaks_event(peaks))?;
    }

    if let Some(result) = &cache.loudness {
        emit(output_tx, &job.token, loudness_event(result))?;
    }

    if let Some(result) = &cache.bpm_key {
        emit(output_tx, &job.token, bpm_event(result))?;
    }

    if cache.complete() {
        return Ok(());
    }

    let need_waveform = cache.waveform.is_none();
    let need_peaks = cache.waveform_peaks.is_none();
    let need_loudness = cache.loudness.is_none();

    if need_waveform || need_peaks || need_loudness {
        let (overview, waveform_peaks, integrated_lufs) =
            decode_overview(job, need_waveform, need_peaks, need_loudness)?;

        if need_waveform {
            cache.waveform = Some(overview);
            cache.waveform_cache_path = None;
        }

        if need_peaks {
            cache.waveform_peaks = waveform_peaks;
        }

        if need_loudness {
            cache.loudness = Some(LoudnessResult { integrated_lufs });
        }

        // Completed stages remain reusable even if the track was switched.
        persist(&job.path, cache_dir, store_tx, &mut cache)?;

        if need_waveform && let Some(peaks) = &cache.waveform {
            emit(output_tx, &job.token, overview_event(peaks))?;
        }

        if need_peaks && let Some(peaks) = &cache.waveform_peaks {
            emit(output_tx, &job.token, waveform_peaks_event(peaks))?;
        }

        if need_loudness && let Some(result) = &cache.loudness {
            emit(output_tx, &job.token, loudness_event(result))?;
        }
    }

    job.token.check()?;

    if cache.bpm_key.is_none() {
        let (samples, sample_rate) = decode_mono(job)?;
        job.token.check()?;

        let result = analyze_bpm_key(&samples, sample_rate, 1).map_err(failed)?;

        drop(samples);

        cache.bpm_key = Some(result);
        persist(&job.path, cache_dir, store_tx, &mut cache)?;

        if let Some(result) = &cache.bpm_key {
            emit(output_tx, &job.token, bpm_event(result))?;
        }
    }

    Ok(())
}

fn read_frame(decoder: &mut impl Iterator<Item = f32>, frame: &mut [f32]) -> bool {
    for sample in frame {
        let Some(value) = decoder.next() else {
            return false;
        };

        *sample = value;
    }

    true
}

fn decode_overview(
    job: &TrackJob,
    need_waveform: bool,
    need_peaks: bool,
    need_loudness: bool,
) -> AnalysisResult<(Vec<f32>, Option<WaveformPeaks>, Option<f32>)> {
    job.token.check()?;

    let mut decoder = open_file_with_decoder(&job.path, SeekAccuracy::Coarse).map_err(failed)?;

    let channels = decoder.channels().get() as usize;
    let sample_rate = decoder.sample_rate().get();

    let estimated_frames = ((job.duration.as_secs_f64() * sample_rate as f64) as usize).max(1);

    let mut peaks = if need_waveform {
        vec![0.0_f32; OVERVIEW_BINS]
    } else {
        Vec::new()
    };

    let mut peak_builder = need_peaks
        .then(|| WaveformPeakBuilder::new(peak_bin_frames(estimated_frames), MAX_PEAK_BINS));

    let mut meter = if need_loudness {
        Some(EbuR128::new(channels as u32, sample_rate, Mode::all()).map_err(failed)?)
    } else {
        None
    };

    let samples_per_update = (sample_rate as usize / 10).max(1) * channels;

    let mut buffer = Vec::new();
    let mut frame = vec![0.0_f32; channels];
    let mut frame_index = 0_usize;

    loop {
        if frame_index.is_multiple_of(CHECK_INTERVAL) {
            job.token.check()?;
            thread::yield_now();
        }

        if !read_frame(&mut decoder, &mut frame) {
            break;
        }

        let mut frame_peak = 0.0_f32;
        let mut frame_min = f32::MAX;
        let mut frame_max = f32::MIN;

        for sample in &frame {
            frame_peak = frame_peak.max(sample.abs());
            frame_min = frame_min.min(*sample);
            frame_max = frame_max.max(*sample);
        }

        if need_waveform {
            let bin = frame_index
                .saturating_mul(OVERVIEW_BINS)
                .checked_div(estimated_frames)
                .unwrap_or(0)
                .min(OVERVIEW_BINS - 1);

            peaks[bin] = peaks[bin].max(frame_peak.clamp(0.0, 1.0));
        }

        if let Some(builder) = peak_builder.as_mut() {
            builder.push(frame_min, frame_max);
        }

        if let Some(meter) = meter.as_mut() {
            buffer.extend_from_slice(&frame);

            if buffer.len() >= samples_per_update {
                meter.add_frames_f32(&buffer).map_err(failed)?;
                buffer.clear();
            }
        }

        frame_index += 1;
    }

    if frame_index == 0 {
        return Err(Failure::Failed("audio contains no samples".into()));
    }

    if let Some(meter) = meter.as_mut()
        && !buffer.is_empty()
    {
        meter.add_frames_f32(&buffer).map_err(failed)?;
    }

    let integrated_lufs = meter
        .and_then(|meter| meter.loudness_global().ok())
        .map(|value| value as f32)
        .filter(|value| value.is_finite());

    let waveform_peaks = peak_builder.and_then(|builder| builder.finish(sample_rate, frame_index));

    Ok((peaks, waveform_peaks, integrated_lufs))
}

fn peak_bin_frames(total_frames: usize) -> usize {
    let required = total_frames.div_ceil(MAX_PEAK_BINS).max(1);

    PEAK_BIN_FRAMES.max(required.next_power_of_two())
}

struct WaveformPeakBuilder {
    bin_frames: usize,
    max_bins: usize,
    frame_index: usize,
    bins: Vec<i8>,
    current_min: f32,
    current_max: f32,
    has_current: bool,
    dropped: bool,
}

impl WaveformPeakBuilder {
    fn new(bin_frames: usize, max_bins: usize) -> Self {
        Self {
            bin_frames: bin_frames.max(1),
            max_bins,
            frame_index: 0,
            bins: Vec::new(),
            current_min: 0.0,
            current_max: 0.0,
            has_current: false,
            dropped: false,
        }
    }

    fn push(&mut self, min: f32, max: f32) {
        if !self.dropped && self.has_current && self.frame_index.is_multiple_of(self.bin_frames) {
            self.flush_bin();
        }

        if !self.dropped {
            if self.has_current {
                self.current_min = self.current_min.min(min);
                self.current_max = self.current_max.max(max);
            } else {
                self.current_min = min;
                self.current_max = max;
                self.has_current = true;
            }
        }

        self.frame_index += 1;
    }

    fn flush_bin(&mut self) {
        if !self.has_current {
            return;
        }

        if self.bins.len() / 2 >= self.max_bins {
            self.dropped = true;
            self.bins.clear();
            self.has_current = false;
            return;
        }

        let min = quantize_peak(self.current_min);
        let max = quantize_peak(self.current_max);
        let (min, max) = if min > max { (0, 0) } else { (min, max) };

        self.bins.push(min);
        self.bins.push(max);
        self.has_current = false;
    }

    fn finish(mut self, sample_rate: u32, total_frames: usize) -> Option<WaveformPeaks> {
        self.flush_bin();

        if self.dropped || total_frames == 0 {
            return None;
        }

        Some(WaveformPeaks {
            sample_rate,
            total_frames,
            bin_frames: self.bin_frames,
            bins: self.bins,
        })
    }
}

fn quantize_peak(value: f32) -> i8 {
    (value.clamp(-1.0, 1.0) * 127.0).round() as i8
}

fn decode_mono(job: &TrackJob) -> AnalysisResult<(Vec<f32>, u32)> {
    job.token.check()?;

    let decoder = open_file_with_decoder(&job.path, SeekAccuracy::Coarse).map_err(failed)?;

    let channels = decoder.channels().get() as usize;
    let sample_rate = decoder.sample_rate().get();

    let limit_secs = job
        .duration
        .as_secs()
        .saturating_add(u64::from(job.duration.subsec_nanos() > 0))
        .max(1);

    let max_frames =
        (sample_rate as usize).saturating_mul(limit_secs.min(usize::MAX as u64) as usize);

    let mut decoder = decoder.amplify_decibel(3.5);
    let mut frame = vec![0.0_f32; channels];
    let mut samples = Vec::new();

    for frame_index in 0..max_frames {
        if frame_index % CHECK_INTERVAL == 0 {
            job.token.check()?;
            thread::yield_now();
        }

        if !read_frame(&mut decoder, &mut frame) {
            break;
        }

        samples.push(frame.iter().sum::<f32>() / channels as f32);
    }

    if samples.is_empty() {
        return Err(Failure::Failed("audio contains no samples".into()));
    }

    Ok((samples, sample_rate))
}

fn analyze_detail(job: &DetailJob, output_tx: &flume::Sender<Output>) -> AnalysisResult<()> {
    job.token.check()?;

    if job.points == 0 || job.start >= job.end {
        return Ok(());
    }

    let mut decoder = open_file_with_decoder(&job.path, SeekAccuracy::Accurate).map_err(failed)?;

    let channels = decoder.channels().get() as usize;
    let sample_rate = decoder.sample_rate().get();

    let total_frames = (job.duration.as_secs_f64() * sample_rate as f64) as usize;

    let start_frame = ((job.start.as_secs_f64() * sample_rate as f64) as usize).min(total_frames);

    let end_frame = ((job.end.as_secs_f64() * sample_rate as f64) as usize).min(total_frames);

    let frame_count = end_frame.saturating_sub(start_frame);

    if frame_count == 0 {
        return Ok(());
    }

    let point_count = job.points.min(frame_count).min(MAX_DETAIL_POINTS);

    let seek_position = Duration::from_secs_f64(start_frame as f64 / sample_rate as f64);

    let seek_succeeded = decoder.try_seek(seek_position).is_ok();

    // A decoder may be left in an undefined state after a failed seek, so the
    // fallback path always starts from a freshly opened stream.
    if !seek_succeeded {
        decoder = open_file_with_decoder(&job.path, SeekAccuracy::Accurate).map_err(failed)?;
    }

    let (points, _frames_read) = decode_detail_window(
        job,
        &mut decoder,
        channels,
        start_frame,
        end_frame,
        frame_count,
        point_count,
        seek_succeeded,
    )?;

    emit(
        output_tx,
        &job.token,
        PlayerEvent::WaveformDetailReady {
            request_id: job.request_id,
            start_frame,
            end_frame,
            total_frames,
            sample_rate,
            points,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn decode_detail_window(
    job: &DetailJob,
    decoder: &mut impl Iterator<Item = f32>,
    channels: usize,
    start_frame: usize,
    end_frame: usize,
    frame_count: usize,
    point_count: usize,
    seek_succeeded: bool,
) -> AnalysisResult<(Vec<WaveformEnvelope>, usize)> {
    let mut points = vec![
        WaveformEnvelope {
            min: 1.0,
            max: -1.0,
        };
        point_count
    ];

    let mut frame = vec![0.0_f32; channels];
    let mut frames_read = 0_usize;

    let first_frame = if seek_succeeded { start_frame } else { 0 };

    for frame_index in first_frame..end_frame {
        if frame_index.is_multiple_of(CHECK_INTERVAL) {
            job.token.check()?;
            thread::yield_now();
        }

        if !read_frame(decoder, &mut frame) {
            break;
        }

        frames_read += 1;

        if frame_index < start_frame {
            continue;
        }

        let point_index = (frame_index - start_frame)
            .saturating_mul(point_count)
            .checked_div(frame_count)
            .unwrap_or(0)
            .min(point_count - 1);

        let point = &mut points[point_index];

        for sample in &frame {
            point.min = point.min.min(*sample);
            point.max = point.max.max(*sample);
        }
    }

    for point in &mut points {
        if point.min > point.max {
            point.min = 0.0;
            point.max = 0.0;
        }
    }

    Ok((points, frames_read))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_jobs_should_be_replaced_by_the_latest_request() {
        let (started_tx, started_rx) = flume::unbounded();
        let (release_tx, release_rx) = flume::bounded(1);
        let (finished_tx, finished_rx) = flume::unbounded();

        let worker = Latest::new(move |value: u32| {
            started_tx.send(value).unwrap();

            if value == 1 {
                release_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            }

            finished_tx.send(value).unwrap();
        });

        worker.submit(1);

        assert_eq!(started_rx.recv_timeout(Duration::from_secs(2)).unwrap(), 1);

        worker.submit(2);
        worker.submit(3);
        release_tx.send(()).unwrap();

        let first = finished_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let second = finished_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        assert_eq!((first, second), (1, 3));
    }

    #[test]
    fn token_should_be_cancelled_after_track_generation_changes() {
        let epoch = Arc::new(AtomicU64::new(1));

        let token = Token {
            track_epoch: epoch.clone(),
            track_generation: 1,
            detail: None,
        };

        epoch.fetch_add(1, Ordering::AcqRel);

        assert!(token.cancelled());
    }

    #[test]
    fn previous_detail_request_should_be_cancelled_independently() {
        let track_epoch = Arc::new(AtomicU64::new(1));
        let detail_epoch = Arc::new(AtomicU64::new(1));

        let token = Token {
            track_epoch,
            track_generation: 1,
            detail: Some((detail_epoch.clone(), 1)),
        };

        detail_epoch.fetch_add(1, Ordering::AcqRel);

        assert!(token.cancelled());
    }

    #[test]
    fn completed_silent_loudness_should_survive_serialization() {
        let cache = CachedAnalysis {
            source: SourceStamp {
                version: CACHE_VERSION,
                path: PathBuf::from("track.wav"),
                length: 100,
                modified_secs: 1,
                modified_nanos: 0,
            },
            waveform: None,
            waveform_peaks: None,
            waveform_cache_path: None,
            bpm_key: None,
            bpm_key_version: BPM_KEY_VERSION,
            loudness: Some(LoudnessResult {
                integrated_lufs: None,
            }),
        };

        let bytes = serde_json::to_vec(&cache).unwrap();
        let restored: CachedAnalysis = serde_json::from_slice(&bytes).unwrap();

        assert!(restored.loudness.is_some());
    }

    #[test]
    fn cache_should_be_rejected_when_source_changes() {
        let source = SourceStamp {
            version: CACHE_VERSION,
            path: PathBuf::from("track.wav"),
            length: 100,
            modified_secs: 1,
            modified_nanos: 0,
        };

        let cache = CachedAnalysis::empty(source.clone());
        let bytes = serde_json::to_vec(&cache).unwrap();

        let changed = SourceStamp {
            length: 200,
            ..source
        };

        assert!(parse_cache(&bytes, &changed).is_none());
    }

    #[test]
    fn old_beat_grid_should_be_reanalyzed_without_discarding_loudness() {
        let source = SourceStamp {
            version: CACHE_VERSION,
            path: PathBuf::from("track.wav"),
            length: 100,
            modified_secs: 1,
            modified_nanos: 0,
        };
        let mut value = serde_json::to_value(CachedAnalysis::empty(source.clone())).unwrap();
        value.as_object_mut().unwrap().remove("bpm_key_version");
        value["bpm_key"] = serde_json::json!({
            "bpm": 120.0,
            "beat_grid": { "offset": 0.0, "beats": [0.0], "bars": [], "downbeats": [] },
            "key_name": "C major",
            "key_numerical": "8B",
        });
        value["loudness"] = serde_json::json!({ "integrated_lufs": -14.0 });
        let bytes = serde_json::to_vec(&value).unwrap();
        let cache = parse_cache(&bytes, &source).unwrap();

        assert!(cache.bpm_key.is_none() && cache.loudness.is_some());
    }

    #[test]
    fn current_beat_grid_should_survive_cache_loading() {
        let source = SourceStamp {
            version: CACHE_VERSION,
            path: PathBuf::from("track.wav"),
            length: 100,
            modified_secs: 1,
            modified_nanos: 0,
        };
        let mut value = serde_json::to_value(CachedAnalysis::empty(source.clone())).unwrap();
        value["bpm_key"] = serde_json::json!({
            "bpm": 120.0,
            "beat_grid": { "offset": 0.137, "beats": [0.137], "bars": [], "downbeats": [] },
            "key_name": "C major",
            "key_numerical": "8B",
        });
        let bytes = serde_json::to_vec(&value).unwrap();
        let cache = parse_cache(&bytes, &source).unwrap();

        assert!(cache.bpm_key.is_some());
    }

    #[test]
    fn partial_results_should_be_merged_without_losing_completed_stages() {
        let source = SourceStamp {
            version: CACHE_VERSION,
            path: PathBuf::from("track.wav"),
            length: 100,
            modified_secs: 1,
            modified_nanos: 0,
        };

        let mut disk = CachedAnalysis::empty(source.clone());
        disk.waveform = Some(vec![0.5; OVERVIEW_BINS]);

        let mut database = CachedAnalysis::empty(source);
        database.loudness = Some(LoudnessResult {
            integrated_lufs: Some(-14.0),
        });

        disk.merge(database);

        assert!(disk.waveform.is_some() && disk.loudness.is_some());
    }

    #[test]
    fn analysis_metadata_should_not_contain_waveform_samples_or_path() {
        let mut cache = CachedAnalysis::empty(SourceStamp {
            version: CACHE_VERSION,
            path: PathBuf::from("track.wav"),
            length: 100,
            modified_secs: 1,
            modified_nanos: 0,
        });

        cache.waveform = Some(vec![0.5; OVERVIEW_BINS]);
        cache.waveform_cache_path = Some(PathBuf::from("wavefroms/ab/example.wfm"));

        let value = serde_json::to_value(cache).unwrap();

        assert!(value.get("waveform").is_none() && value.get("waveform_cache_path").is_none());
    }

    fn test_token() -> Token {
        let epoch = Arc::new(AtomicU64::new(0));

        Token {
            track_epoch: epoch,
            track_generation: 0,
            detail: None,
        }
    }

    fn write_test_wav(
        name: &str,
        sample_rate: u32,
        frames: usize,
        value: impl Fn(usize) -> f32,
    ) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "wave_player_{name}_{}_{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(&path, spec).unwrap();

        for frame in 0..frames {
            let sample = (value(frame).clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(sample).unwrap();
        }

        writer.finalize().unwrap();
        path
    }

    #[test]
    fn peak_bin_frames_should_stay_fine_for_normal_tracks() {
        let five_minutes = 5 * 60 * 44_100;
        let one_hour = 60 * 60 * 44_100;

        assert_eq!(peak_bin_frames(five_minutes), PEAK_BIN_FRAMES);
        assert_eq!(five_minutes.div_ceil(PEAK_BIN_FRAMES), 103_360);

        assert_eq!(peak_bin_frames(one_hour), PEAK_BIN_FRAMES);
        assert_eq!(one_hour.div_ceil(PEAK_BIN_FRAMES), 1_240_313);
    }

    #[test]
    fn peak_bin_frames_should_grow_for_extremely_long_tracks() {
        let ten_hours = 10 * 60 * 60 * 44_100;
        let bin_frames = peak_bin_frames(ten_hours);

        assert_eq!(bin_frames, 512);
        assert!(bin_frames.is_power_of_two());
        assert!(ten_hours.div_ceil(bin_frames) <= MAX_PEAK_BINS);
    }

    #[test]
    fn peak_builder_should_quantize_min_max_pairs() {
        let mut builder = WaveformPeakBuilder::new(2, MAX_PEAK_BINS);

        builder.push(-0.75, 0.75);
        builder.push(-0.5, 0.5);
        builder.push(0.25, 0.25);

        let peaks = builder.finish(8_000, 3).unwrap();

        assert_eq!(peaks.bin_frames, 2);
        assert_eq!(peaks.total_frames, 3);
        assert_eq!(peaks.bins, vec![-95, 95, 32, 32]);
    }

    #[test]
    fn peak_builder_should_keep_pyramid_below_limit() {
        let mut builder = WaveformPeakBuilder::new(1, 3);

        for _ in 0..3 {
            builder.push(-0.5, 0.5);
        }

        let peaks = builder.finish(44_100, 3).unwrap();

        assert_eq!(peaks.bins.len(), 6);
    }

    #[test]
    fn peak_builder_should_drop_pyramid_above_bin_limit() {
        let mut builder = WaveformPeakBuilder::new(1, 2);

        for _ in 0..3 {
            builder.push(-0.5, 0.5);
        }

        assert!(builder.finish(44_100, 3).is_none());
    }

    #[test]
    fn decode_overview_should_build_overview_and_peak_pyramid_in_one_pass() {
        let sample_rate = 8_000;
        let total_frames = 4_000;
        let path = write_test_wav("overview", sample_rate, total_frames, |frame| match frame {
            500 => 0.75,
            501 => -0.75,
            _ => 0.0,
        });

        let job = TrackJob {
            path: path.clone(),
            duration: Duration::from_secs_f32(total_frames as f32 / sample_rate as f32),
            token: test_token(),
            force: false,
        };

        let (overview, peaks, loudness) = decode_overview(&job, true, true, false).unwrap();
        let peaks = peaks.expect("peak pyramid should be built");

        assert_eq!(overview.len(), OVERVIEW_BINS);
        assert!(overview.iter().any(|peak| *peak > 0.5));
        assert!(loudness.is_none());

        assert_eq!(peaks.sample_rate, sample_rate);
        assert_eq!(peaks.total_frames, total_frames);
        assert_eq!(peaks.bin_frames, PEAK_BIN_FRAMES);
        assert_eq!(peaks.bins.len(), total_frames.div_ceil(PEAK_BIN_FRAMES) * 2);
        assert_eq!((peaks.bins[6], peaks.bins[7]), (-95, 95));

        let event = overview_event(&overview);
        let PlayerEvent::WaveformChunkReady {
            chunk,
            start_bin,
            total_bins,
        } = event
        else {
            panic!("overview event variant changed");
        };

        assert_eq!(
            (start_bin, total_bins, chunk.len()),
            (0, OVERVIEW_BINS, 500)
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn waveform_peaks_payload_should_roundtrip_base64() {
        let peaks = WaveformPeaks {
            sample_rate: 48_000,
            total_frames: 10,
            bin_frames: PEAK_BIN_FRAMES,
            bins: vec![-127, 127, -10, 20, 0, 0],
        };

        let event = waveform_peaks_event(&peaks);
        let PlayerEvent::WaveformPeaksReady {
            sample_rate,
            total_frames,
            bin_frames,
            encoded,
        } = event
        else {
            panic!("peak event variant changed");
        };

        assert_eq!((sample_rate, total_frames, bin_frames), (48_000, 10, 128));

        let bytes = STANDARD.decode(encoded.as_bytes()).unwrap();
        let decoded: Vec<i8> = bytes.iter().map(|byte| *byte as i8).collect();

        assert_eq!(decoded, peaks.bins);
    }

    #[test]
    fn peak_event_clone_should_share_encoded_buffer() {
        let peaks = WaveformPeaks {
            sample_rate: 48_000,
            total_frames: 10,
            bin_frames: PEAK_BIN_FRAMES,
            bins: vec![-127, 127],
        };

        let event = waveform_peaks_event(&peaks);
        let cloned = event.clone();

        let (
            PlayerEvent::WaveformPeaksReady { encoded: left, .. },
            PlayerEvent::WaveformPeaksReady { encoded: right, .. },
        ) = (&event, &cloned)
        else {
            panic!("peak event variant changed");
        };

        assert!(Arc::ptr_eq(left, right));
    }

    #[test]
    fn replay_should_resend_overview_and_peak_pyramid() {
        let (status_tx, mut status_rx) = async_broadcast::broadcast(8);
        let (_output_tx, output_rx) = flume::bounded(8);

        let peaks = WaveformPeaks {
            sample_rate: 48_000,
            total_frames: 10,
            bin_frames: PEAK_BIN_FRAMES,
            bins: vec![-127, 127],
        };

        let service = AnalysisService {
            track_epoch: Arc::new(AtomicU64::new(0)),
            detail_epoch: Arc::new(AtomicU64::new(0)),
            tracks: Latest::new(|_job: TrackJob| {}),
            details: Latest::new(|_job: DetailJob| {}),
            output_rx,
            overview: Some(vec![0.5; OVERVIEW_BINS]),
            waveform_peaks: Some(waveform_peaks_event(&peaks)),
        };

        service.replay_overview(&status_tx);

        let first = status_rx.try_recv().unwrap();
        let second = status_rx.try_recv().unwrap();

        assert!(matches!(first, PlayerEvent::WaveformChunkReady { .. }));
        assert!(matches!(second, PlayerEvent::WaveformPeaksReady { .. }));
    }

    #[test]
    fn waveform_peaks_should_roundtrip_cache_conversion() {
        let peaks = WaveformPeaks {
            sample_rate: 44_100,
            total_frames: 128_000,
            bin_frames: PEAK_BIN_FRAMES,
            bins: vec![-127, 127, -10, 20],
        };

        let cached = cached_waveform_peaks(&peaks);

        assert_eq!(cached.sample_rate, peaks.sample_rate);
        assert_eq!(cached.total_frames as usize, peaks.total_frames);
        assert_eq!(cached.bin_frames as usize, peaks.bin_frames);
        assert_eq!(cached.bins, peaks.bins);

        let restored = waveform_peaks_from_cache(cached);

        assert_eq!(restored, peaks);
    }

    #[test]
    fn publish_should_store_overview_and_peak_payload_for_replay() {
        let (output_tx, output_rx) = flume::bounded(8);
        let (status_tx, mut status_rx) = async_broadcast::broadcast(8);

        let peaks = WaveformPeaks {
            sample_rate: 44_100,
            total_frames: 128_000,
            bin_frames: PEAK_BIN_FRAMES,
            bins: vec![-127, 127],
        };

        let mut service = AnalysisService {
            track_epoch: Arc::new(AtomicU64::new(0)),
            detail_epoch: Arc::new(AtomicU64::new(0)),
            tracks: Latest::new(|_job: TrackJob| {}),
            details: Latest::new(|_job: DetailJob| {}),
            output_rx,
            overview: None,
            waveform_peaks: None,
        };

        output_tx
            .send(Output {
                token: test_token(),
                event: overview_event(&[0.5; OVERVIEW_BINS]),
            })
            .unwrap();

        output_tx
            .send(Output {
                token: test_token(),
                event: waveform_peaks_event(&peaks),
            })
            .unwrap();

        service.publish(&status_tx);

        assert!(service.overview.is_some());
        assert!(matches!(
            service.waveform_peaks,
            Some(PlayerEvent::WaveformPeaksReady { .. })
        ));
        assert!(matches!(
            status_rx.try_recv().unwrap(),
            PlayerEvent::WaveformChunkReady { .. }
        ));
        assert!(matches!(
            status_rx.try_recv().unwrap(),
            PlayerEvent::WaveformPeaksReady { .. }
        ));
    }

    fn detail_job(path: PathBuf, start: Duration, end: Duration) -> DetailJob {
        DetailJob {
            path,
            duration: Duration::from_secs(4),
            request_id: 7,
            start,
            end,
            points: 512,
            token: test_token(),
        }
    }

    #[test]
    fn analyze_detail_should_emit_aligned_window_event() {
        let sample_rate = 8_000_usize;
        let total_frames = 4 * sample_rate;
        let marker = 2 * sample_rate + 1_234;
        let path = write_test_wav(
            "analyze_detail",
            sample_rate as u32,
            total_frames,
            |frame| match frame {
                frame if frame == marker => 0.8,
                _ => 0.0,
            },
        );

        let job = detail_job(path.clone(), Duration::from_secs(2), Duration::from_secs(3));
        let (output_tx, output_rx) = flume::bounded(8);

        analyze_detail(&job, &output_tx).unwrap();

        let output = output_rx.try_recv().unwrap();
        let PlayerEvent::WaveformDetailReady {
            request_id,
            start_frame,
            end_frame,
            total_frames: emitted_total_frames,
            sample_rate: emitted_sample_rate,
            points,
        } = output.event
        else {
            panic!("detail event variant changed");
        };

        assert_eq!(request_id, 7);
        assert_eq!(start_frame, 2 * sample_rate);
        assert_eq!(end_frame, 3 * sample_rate);
        assert_eq!(emitted_total_frames, total_frames);
        assert_eq!(emitted_sample_rate, sample_rate as u32);
        assert_eq!(points.len(), 512);

        let point_index = (marker - start_frame) * points.len() / (end_frame - start_frame);

        assert!(points[point_index].max > 0.7);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn detail_window_should_decode_only_the_requested_frames() {
        let sample_rate = 8_000_usize;
        let total_frames = 4 * sample_rate;
        let marker = 2 * sample_rate + 1_234;
        let path = write_test_wav(
            "detail",
            sample_rate as u32,
            total_frames,
            |frame| match frame {
                frame if frame == marker => 0.8,
                frame if frame == marker + 1 => -0.6,
                _ => 0.0,
            },
        );

        let job = detail_job(path.clone(), Duration::from_secs(2), Duration::from_secs(3));

        let start_frame = 2 * sample_rate;
        let end_frame = 3 * sample_rate;
        let frame_count = sample_rate;
        let point_count = 512;

        let mut decoder = open_file_with_decoder(&path, SeekAccuracy::Accurate).unwrap();
        let seek_succeeded = decoder.try_seek(Duration::from_secs(2)).is_ok();

        assert!(seek_succeeded, "WAV decoder should support accurate seek");

        let (points, frames_read) = decode_detail_window(
            &job,
            &mut decoder,
            1,
            start_frame,
            end_frame,
            frame_count,
            point_count,
            seek_succeeded,
        )
        .unwrap();

        assert_eq!(frames_read, frame_count);
        assert_eq!(points.len(), point_count);

        let point_index = (marker - start_frame) * point_count / frame_count;

        assert!(points[point_index].max > 0.7);
        assert!(points[point_index].min > -0.1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn detail_window_fallback_should_align_with_overview() {
        let sample_rate = 8_000_usize;
        let total_frames = 4 * sample_rate;
        let marker = 2 * sample_rate + 1_234;
        let path = write_test_wav(
            "detail_fallback",
            sample_rate as u32,
            total_frames,
            |frame| match frame {
                frame if frame == marker => 0.8,
                frame if frame == marker + 1 => -0.6,
                _ => 0.0,
            },
        );

        let job = detail_job(path.clone(), Duration::from_secs(2), Duration::from_secs(3));

        let start_frame = 2 * sample_rate;
        let end_frame = 3 * sample_rate;
        let frame_count = sample_rate;
        let point_count = 512;

        let mut decoder = open_file_with_decoder(&path, SeekAccuracy::Accurate).unwrap();

        let (points, frames_read) = decode_detail_window(
            &job,
            &mut decoder,
            1,
            start_frame,
            end_frame,
            frame_count,
            point_count,
            false,
        )
        .unwrap();

        assert_eq!(frames_read, end_frame);

        let point_index = (marker - start_frame) * point_count / frame_count;

        assert!(points[point_index].max > 0.7);

        let _ = std::fs::remove_file(path);
    }
}
