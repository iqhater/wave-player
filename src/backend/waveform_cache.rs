use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

pub(crate) const CACHE_VERSION: u32 = 2;

const MAGIC: &[u8; 8] = b"WPWFM001";
const WAVEFORM_DIR: &str = "wavefroms";
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
) -> io::Result<Option<Vec<f32>>> {
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

    let mut trailing = [0_u8; 1];

    if reader.read(&mut trailing)? != 0 {
        return Err(invalid_data("unexpected waveform cache trailing data"));
    }

    Ok(Some(peaks))
}

pub(crate) fn save(cache_dir: &Path, source: &SourceStamp, peaks: &[f32]) -> io::Result<PathBuf> {
    if peaks.is_empty()
        || peaks.len() > MAX_POINTS
        || peaks
            .iter()
            .any(|peak| !peak.is_finite() || !(0.0..=1.0).contains(peak))
    {
        return Err(invalid_data("invalid waveform peaks"));
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
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn waveform_should_roundtrip_through_file_cache() {
        let fixture = Fixture::new();
        let peaks = vec![0.0, 0.25, 0.5, 1.0];

        let relative = save(&fixture.root, &fixture.source, &peaks).unwrap();

        let restored = load(&fixture.root, &relative, &fixture.source, peaks.len()).unwrap();

        assert_eq!(restored, Some(peaks));
    }

    #[test]
    fn waveform_should_be_rejected_when_source_changes() {
        let fixture = Fixture::new();

        let relative = save(&fixture.root, &fixture.source, &[0.5]).unwrap();

        let changed = SourceStamp {
            length: fixture.source.length + 1,
            ..fixture.source.clone()
        };

        let restored = load(&fixture.root, &relative, &changed, 1).unwrap();

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

        let relative = save(&fixture.root, &fixture.source, &[0.5]).unwrap();

        let absolute = fixture.root.join(&relative);
        let mut bytes = fs::read(&absolute).unwrap();
        bytes.pop();
        fs::write(&absolute, bytes).unwrap();

        let result = load(&fixture.root, &relative, &fixture.source, 1);

        assert!(result.is_err());
    }
}
