use std::fs;
use std::path::{Path, PathBuf};

/// Returns a stable cache path for a track cover.
pub fn path_for_track(cache_dir: &Path, track_path: &Path) -> PathBuf {
    let key = stable_track_key(track_path);

    cache_dir.join("covers").join(format!("{key:016x}.bin"))
}

/// Loads a cached cover for a track.
pub fn load_cover(cache_dir: &Path, track_path: &Path) -> Option<Vec<u8>> {
    let path = path_for_track(cache_dir, track_path);

    fs::read(path).ok().filter(|bytes| !bytes.is_empty())
}

/// Saves a cover and returns the saved path.
pub fn save_cover(cache_dir: &Path, track_path: &Path, bytes: &[u8]) -> Option<PathBuf> {
    if bytes.is_empty() {
        return None;
    }

    let path = path_for_track(cache_dir, track_path);

    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    fs::write(&path, bytes).ok()?;

    Some(path)
}

/// FNV-1a hash.
///
/// A custom stable hash is used instead of DefaultHasher because
/// cache names must remain stable between application launches.
fn stable_track_key(track_path: &Path) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;

    for byte in track_path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_is_stable() {
        let cache_dir = PathBuf::from("cache");
        let track_path = PathBuf::from("music/song.mp3");

        assert_eq!(
            path_for_track(&cache_dir, &track_path),
            path_for_track(&cache_dir, &track_path)
        );
    }

    #[test]
    fn different_tracks_have_different_paths() {
        let cache_dir = PathBuf::from("cache");

        let first = path_for_track(&cache_dir, Path::new("music/first.mp3"));
        let second = path_for_track(&cache_dir, Path::new("music/second.mp3"));

        assert_ne!(first, second);
    }
}
