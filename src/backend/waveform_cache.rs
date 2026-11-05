use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::backend::analysis::service::MAX_PEAK_BINS;

pub(crate) const CACHE_VERSION: u32 = 3;

const MAGIC: &[u8; 8] = b"WPWFM001";
pub(crate) const WAVEFORM_DIR: &str = "wavefroms";
const MAX_HEADER_BYTES: usize = 128 * 1024;
const MAX_POINTS: usize = 1_000_000;

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SourceStamp {
    pub(crate) version: u32,
    pub(crate) path: PathBuf,
    pub(crate) length: u64,
    pub(crate) modified_secs: u64,
    pub(crate) modified_nanos: u32,
}

impl SourceStamp {
    pub(crate) fn read(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(invalid_data)?;

        Ok(Self {
            version: CACHE_VERSION,
            path: fs::canonicalize(path)?,
            length: metadata.len(),
            modified_secs: modified.as_secs(),
            modified_nanos: modified.subsec_nanos(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CachedWaveform {
    pub(crate) peaks: Vec<f32>,
    pub(crate) waveform_peaks: Option<CachedWaveformPeaks>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CachedWaveformPeaks {
    pub(crate) sample_rate: u32,
    pub(crate) total_frames: u64,
    pub(crate) bin_frames: u32,
    /// Packed `min`/`max` pairs for each bin.
    pub(crate) bins: Vec<i8>,
}

fn invalid_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn resolve(cache_dir: &Path, relative: &Path) -> io::Result<PathBuf> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_data("invalid relative waveform cache path"));
    }

    Ok(cache_dir.join(relative))
}

pub(crate) fn relative_path(source: &SourceStamp) -> io::Result<PathBuf> {
    let bytes = serde_json::to_vec(source).map_err(invalid_data)?;

    let mut hash = 0xcbf29ce484222325_u64;

    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    let key = format!("{hash:016x}");

    // A versioned filename avoids overwriting files using an older codec.
    Ok(PathBuf::from(WAVEFORM_DIR)
        .join(&key[..2])
        .join(format!("{key}.v1.wfm")))
}

pub(crate) fn load(
    cache_dir: &Path,
    relative: &Path,
    expected_source: &SourceStamp,
    expected_points: usize,
) -> io::Result<Option<CachedWaveform>> {
    if expected_points == 0 || expected_points > MAX_POINTS {
        return Err(invalid_data("invalid expected waveform point count"));
    }

    let path = resolve(cache_dir, relative)?;

    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };

    let mut reader = BufReader::new(file);

    let mut magic = [0_u8; 8];
    reader.read_exact(&mut magic)?;

    if &magic != MAGIC {
        return Err(invalid_data("unsupported waveform cache format"));
    }

    let header_len = read_u32(&mut reader)? as usize;

    if header_len == 0 || header_len > MAX_HEADER_BYTES {
        return Err(invalid_data("invalid waveform cache header size"));
    }

    let mut header = vec![0_u8; header_len];
    reader.read_exact(&mut header)?;

    let source: SourceStamp = serde_json::from_slice(&header).map_err(invalid_data)?;

    if &source != expected_source {
        return Ok(None);
    }

    let point_count = read_u32(&mut reader)? as usize;

    if point_count != expected_points {
        return Ok(None);
    }

    let mut peaks = Vec::with_capacity(point_count);

    for _ in 0..point_count {
        let mut bytes = [0_u8; 4];
        reader.read_exact(&mut bytes)?;

        let peak = f32::from_le_bytes(bytes);

        if !peak.is_finite() || !(0.0..=1.0).contains(&peak) {
            return Err(invalid_data("invalid waveform peak"));
        }

        peaks.push(peak);
    }

    let sample_rate = read_u32(&mut reader)?;
    let total_frames = read_u64(&mut reader)?;
    let bin_frames = read_u32(&mut reader)?;
    let bin_count = read_u32(&mut reader)? as usize;

    if sample_rate == 0 {
        return Err(invalid_data("invalid waveform peak sample rate"));
    }

    if total_frames == 0 {
        return Err(invalid_data("invalid waveform peak frame count"));
    }

    if bin_frames == 0 {
        return Err(invalid_data("invalid waveform peak bin size"));
    }

    if bin_count == 0 || bin_count > MAX_PEAK_BINS {
        return Err(invalid_data("invalid waveform peak bin count"));
    }

    let byte_count = bin_count
        .checked_mul(2)
        .ok_or_else(|| invalid_data("waveform peak data is too large"))?;

    let mut bytes = vec![0_u8; byte_count];
    reader.read_exact(&mut bytes)?;

    let mut bins = Vec::with_capacity(byte_count);

    for byte in bytes {
        bins.push(byte as i8);
    }

    let mut trailing = [0_u8; 1];

    if reader.read(&mut trailing)? != 0 {
        return Err(invalid_data("unexpected waveform cache trailing data"));
    }

    Ok(Some(CachedWaveform {
        peaks,
        waveform_peaks: Some(CachedWaveformPeaks {
            sample_rate,
            total_frames,
            bin_frames,
            bins,
        }),
    }))
}

pub(crate) fn save(
    cache_dir: &Path,
    source: &SourceStamp,
    waveform: &CachedWaveform,
) -> io::Result<PathBuf> {
    let peaks = &waveform.peaks;

    if peaks.is_empty()
        || peaks.len() > MAX_POINTS
        || peaks
            .iter()
            .any(|peak| !peak.is_finite() || !(0.0..=1.0).contains(peak))
    {
        return Err(invalid_data("invalid waveform peaks"));
    }

    let waveform_peaks = waveform
        .waveform_peaks
        .as_ref()
        .ok_or_else(|| invalid_data("waveform peak pyramid is missing"))?;

    if waveform_peaks.sample_rate == 0
        || waveform_peaks.total_frames == 0
        || waveform_peaks.bin_frames == 0
        || waveform_peaks.bins.is_empty()
        || !waveform_peaks.bins.len().is_multiple_of(2)
        || waveform_peaks.bins.len() / 2 > MAX_PEAK_BINS
    {
        return Err(invalid_data("invalid waveform peak pyramid"));
    }

    let header = serde_json::to_vec(source).map_err(invalid_data)?;

    if header.len() > MAX_HEADER_BYTES {
        return Err(invalid_data("waveform cache header is too large"));
    }

    let relative = relative_path(source)?;
    let path = resolve(cache_dir, &relative)?;

    let parent = path
        .parent()
        .ok_or_else(|| invalid_data("waveform cache path has no parent"))?;

    fs::create_dir_all(parent)?;

    let temporary = path.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));

    let result = (|| -> io::Result<()> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;

        let mut writer = BufWriter::new(file);

        writer.write_all(MAGIC)?;
        writer.write_all(&(header.len() as u32).to_le_bytes())?;
        writer.write_all(&header)?;
        writer.write_all(&(peaks.len() as u32).to_le_bytes())?;

        for peak in peaks {
            writer.write_all(&peak.to_le_bytes())?;
        }

        writer.write_all(&waveform_peaks.sample_rate.to_le_bytes())?;
        writer.write_all(&waveform_peaks.total_frames.to_le_bytes())?;
        writer.write_all(&waveform_peaks.bin_frames.to_le_bytes())?;
        writer.write_all(&((waveform_peaks.bins.len() / 2) as u32).to_le_bytes())?;

        for bin in &waveform_peaks.bins {
            writer.write_all(&[*bin as u8])?;
        }

        writer.flush()?;
        writer.get_ref().sync_all()?;
        drop(writer);

        fs::rename(&temporary, &path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    result?;

    Ok(relative)
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
        source: SourceStamp,
    }

    impl Fixture {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();

            let root = std::env::temp_dir().join(format!(
                "wave-player-cache-{}-{unique}-{}",
                std::process::id(),
                NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
            ));

            fs::create_dir_all(&root).unwrap();

            let audio = root.join("track.raw");
            fs::write(&audio, [0_u8; 16]).unwrap();

            let source = SourceStamp::read(&audio).unwrap();

            Self { root, source }
        }

        fn waveform(&self) -> CachedWaveform {
            CachedWaveform {
                peaks: vec![0.0, 0.25, 0.5, 1.0],
                waveform_peaks: Some(CachedWaveformPeaks {
                    sample_rate: 44_100,
                    total_frames: 8_000,
                    bin_frames: 128,
                    bins: vec![-127, 127, -10, 20, 0, 0],
                }),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn metadata_offset(bytes: &[u8]) -> usize {
        let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let point_count =
            u32::from_le_bytes(bytes[12 + header_len..16 + header_len].try_into().unwrap())
                as usize;

        16 + header_len + point_count * 4
    }

    #[test]
    fn waveform_should_roundtrip_through_file_cache() {
        let fixture = Fixture::new();
        let waveform = fixture.waveform();

        let relative = save(&fixture.root, &fixture.source, &waveform).unwrap();

        let restored = load(
            &fixture.root,
            &relative,
            &fixture.source,
            waveform.peaks.len(),
        )
        .unwrap();

        assert_eq!(restored, Some(waveform));
    }

    #[test]
    fn waveform_should_be_rejected_when_source_changes() {
        let fixture = Fixture::new();

        let relative = save(&fixture.root, &fixture.source, &fixture.waveform()).unwrap();

        let changed = SourceStamp {
            length: fixture.source.length + 1,
            ..fixture.source.clone()
        };

        let restored = load(&fixture.root, &relative, &changed, 4).unwrap();

        assert!(restored.is_none());
    }

    #[test]
    fn old_cache_version_should_be_ignored() {
        let fixture = Fixture::new();

        let relative = save(&fixture.root, &fixture.source, &fixture.waveform()).unwrap();

        let old_source = SourceStamp {
            version: 2,
            ..fixture.source.clone()
        };

        let restored = load(&fixture.root, &relative, &old_source, 4).unwrap();

        assert!(restored.is_none());
    }

    #[test]
    fn waveform_loader_should_reject_parent_directory_paths() {
        let fixture = Fixture::new();

        let result = load(
            &fixture.root,
            Path::new("../outside.wfm"),
            &fixture.source,
            1,
        );

        assert!(result.is_err());
    }

    #[test]
    fn waveform_loader_should_reject_truncated_files() {
        let fixture = Fixture::new();

        let relative = save(&fixture.root, &fixture.source, &fixture.waveform()).unwrap();

        let absolute = fixture.root.join(&relative);
        let mut bytes = fs::read(&absolute).unwrap();
        bytes.pop();
        fs::write(&absolute, bytes).unwrap();

        let result = load(&fixture.root, &relative, &fixture.source, 4);

        assert!(result.is_err());
    }

    #[test]
    fn waveform_loader_should_reject_invalid_peak_metadata() {
        let fixture = Fixture::new();
        let waveform = fixture.waveform();

        for (offset, value) in [(0, 0_u32), (12, 0_u32), (16, u32::MAX)] {
            let relative = save(&fixture.root, &fixture.source, &waveform).unwrap();
            let absolute = fixture.root.join(&relative);
            let mut bytes = fs::read(&absolute).unwrap();
            let metadata = metadata_offset(&bytes);

            bytes[metadata + offset..metadata + offset + 4].copy_from_slice(&value.to_le_bytes());
            fs::write(&absolute, bytes).unwrap();

            let result = load(&fixture.root, &relative, &fixture.source, 4);

            assert!(
                result.is_err(),
                "metadata at offset {offset} should be rejected"
            );
        }
    }

    #[test]
    fn waveform_loader_should_reject_trailing_data() {
        let fixture = Fixture::new();

        let relative = save(&fixture.root, &fixture.source, &fixture.waveform()).unwrap();

        let absolute = fixture.root.join(&relative);
        let mut bytes = fs::read(&absolute).unwrap();
        bytes.push(0);
        fs::write(&absolute, bytes).unwrap();

        let result = load(&fixture.root, &relative, &fixture.source, 4);

        assert!(result.is_err());
    }

    #[test]
    fn waveform_save_should_fail_without_peak_pyramid() {
        let fixture = Fixture::new();
        let waveform = CachedWaveform {
            peaks: vec![0.5],
            waveform_peaks: None,
        };

        let result = save(&fixture.root, &fixture.source, &waveform);

        assert!(result.is_err());
    }

    #[test]
    fn waveform_save_should_report_unwritable_cache_directory() {
        let fixture = Fixture::new();

        fs::write(fixture.root.join(WAVEFORM_DIR), [0_u8; 1]).unwrap();

        let result = save(&fixture.root, &fixture.source, &fixture.waveform());

        assert!(result.is_err());
    }
}
