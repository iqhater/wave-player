use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use ebur128::{EbuR128, Mode};
use rodio::Source;
use serde::{Deserialize, Serialize};

use super::bpm_key::{BPM_KEY_VERSION, BpmKeyResult, analyze_bpm_key};
use crate::backend::decoder::open_file_with_decoder;
use crate::backend::waveform_cache::{self, SourceStamp};
use crate::shared::command_types::CachedTrackAnalysis;
use crate::shared::command_types::StoreCommand;
use crate::shared::event_types::{PlayerEvent, StatusSender, WaveformEnvelope};

#[cfg(test)]
use crate::backend::waveform_cache::CACHE_VERSION;

pub(crate) const ANALYSIS_DIR: &str = "analysis";
const OVERVIEW_BINS: usize = 500;
const MAX_DETAIL_POINTS: usize = 16_384;
const CHECK_INTERVAL: usize = 4096;

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
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.track_epoch.fetch_add(1, Ordering::AcqRel);
        self.detail_epoch.fetch_add(1, Ordering::AcqRel);
        self.overview = None;
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

            if let PlayerEvent::WaveformChunkReady { chunk, .. } = &output.event {
                self.overview = Some(chunk.clone());
            }

            // Publication and track switching run on the same engine thread.
            let _ = status_tx.try_broadcast(output.event);
        }
    }

    pub(crate) fn replay_overview(&self, status_tx: &StatusSender) {
        if let Some(peaks) = &self.overview {
            let _ = status_tx.try_broadcast(overview_event(peaks));
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
            waveform_cache_path: None,
            bpm_key: None,
            bpm_key_version: BPM_KEY_VERSION,
            loudness: None,
        }
    }

    fn complete(&self) -> bool {
        self.waveform.is_some() && self.bpm_key.is_some() && self.loudness.is_some()
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

    if cache.waveform_cache_path.is_none() {
        if let Some(peaks) = &cache.waveform {
            match waveform_cache::save(cache_dir, &cache.source, peaks) {
                Ok(relative) => {
                    cache.waveform_cache_path = Some(relative);
                }
                Err(error) => {
                    eprintln!("failed to save waveform for {}: {error}", path.display());
                }
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
            Ok(Some(peaks)) => {
                cache.waveform = Some(peaks);
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
    let need_loudness = cache.loudness.is_none();

    if need_waveform || need_loudness {
        let (peaks, integrated_lufs) = decode_overview(job, need_waveform, need_loudness)?;

        if need_waveform {
            cache.waveform = Some(peaks);
            cache.waveform_cache_path = None;
        }

        if need_loudness {
            cache.loudness = Some(LoudnessResult { integrated_lufs });
        }

        // Completed stages remain reusable even if the track was switched.
        persist(&job.path, cache_dir, store_tx, &mut cache)?;

        if need_waveform {
            if let Some(peaks) = &cache.waveform {
                emit(output_tx, &job.token, overview_event(peaks))?;
            }
        }

        if need_loudness {
            if let Some(result) = &cache.loudness {
                emit(output_tx, &job.token, loudness_event(result))?;
            }
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
    need_loudness: bool,
) -> AnalysisResult<(Vec<f32>, Option<f32>)> {
    job.token.check()?;

    let mut decoder = open_file_with_decoder(&job.path).map_err(failed)?;

    let channels = decoder.channels().get() as usize;
    let sample_rate = decoder.sample_rate().get();

    let estimated_frames = ((job.duration.as_secs_f64() * sample_rate as f64) as usize).max(1);

    let mut peaks = if need_waveform {
        vec![0.0_f32; OVERVIEW_BINS]
    } else {
        Vec::new()
    };

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
        if frame_index % CHECK_INTERVAL == 0 {
            job.token.check()?;
        }

        if !read_frame(&mut decoder, &mut frame) {
            break;
        }

        if need_waveform {
            let bin = frame_index
                .saturating_mul(OVERVIEW_BINS)
                .checked_div(estimated_frames)
                .unwrap_or(0)
                .min(OVERVIEW_BINS - 1);

            let peak = frame
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
                .clamp(0.0, 1.0);

            peaks[bin] = peaks[bin].max(peak);
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

    if let Some(meter) = meter.as_mut() {
        if !buffer.is_empty() {
            meter.add_frames_f32(&buffer).map_err(failed)?;
        }
    }

    let integrated_lufs = meter
        .and_then(|meter| meter.loudness_global().ok())
        .map(|value| value as f32)
        .filter(|value| value.is_finite());

    Ok((peaks, integrated_lufs))
}

fn decode_mono(job: &TrackJob) -> AnalysisResult<(Vec<f32>, u32)> {
    job.token.check()?;

    let decoder = open_file_with_decoder(&job.path).map_err(failed)?;

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

    let mut decoder = open_file_with_decoder(&job.path).map_err(failed)?;

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

    let mut points = vec![
        WaveformEnvelope {
            min: 1.0,
            max: -1.0,
        };
        point_count
    ];

    let mut frame = vec![0.0_f32; channels];

    for frame_index in 0..end_frame {
        if frame_index % CHECK_INTERVAL == 0 {
            job.token.check()?;
        }

        if !read_frame(&mut decoder, &mut frame) {
            break;
        }

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
}
