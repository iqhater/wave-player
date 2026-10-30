use crate::shared::SUPPORTED_AUDIO_EXTENSIONS;
use fs4::{FileExt, TryLockError};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

const LOCK_FILE: &str = "instance.lock";
const OPEN_REQUEST_FILE: &str = "open_request.json";
const WATCH_INTERVAL: Duration = Duration::from_millis(100);

pub struct InstanceLock {
    _file: File,
}

pub enum InstanceLockState {
    Acquired(InstanceLock),
    AlreadyRunning,
    Unavailable(std::io::Error),
}

pub fn is_supported_audio_path(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                SUPPORTED_AUDIO_EXTENSIONS
                    .iter()
                    .any(|supported| supported.eq_ignore_ascii_case(extension))
            })
}

pub fn first_audio_file_arg<I: IntoIterator<Item = OsString>>(args: I) -> Option<PathBuf> {
    args.into_iter()
        .skip(1)
        .filter_map(|argument| {
            let text = argument.to_string_lossy();

            if text.starts_with('-') {
                None
            } else {
                Some(PathBuf::from(argument))
            }
        })
        .find(|path| is_supported_audio_path(path))
}

use std::sync::atomic::{AtomicU64, Ordering};

static OPEN_REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
struct OpenRequest {
    path: String,
}

pub fn write_open_request(directory: &Path, path: &Path) -> std::io::Result<()> {
    let target = directory.join(OPEN_REQUEST_FILE);
    let temporary = directory.join(format!(
        "{OPEN_REQUEST_FILE}.{}.{}.tmp",
        std::process::id(),
        OPEN_REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let request = OpenRequest {
        path: path.to_string_lossy().into_owned(),
    };
    let bytes = serde_json::to_vec(&request).map_err(std::io::Error::other)?;

    fs::write(&temporary, bytes)?;

    if let Err(error) = fs::rename(&temporary, &target) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    Ok(())
}

pub fn take_open_request(directory: &Path) -> Option<PathBuf> {
    let request_path = directory.join(OPEN_REQUEST_FILE);
    let bytes = fs::read(&request_path).ok()?;
    let _ = fs::remove_file(&request_path);

    let request: OpenRequest = serde_json::from_slice(&bytes).ok()?;
    let path = PathBuf::from(request.path);

    is_supported_audio_path(&path).then_some(path)
}

pub fn spawn_open_request_watcher(directory: PathBuf, sender: flume::Sender<PathBuf>) {
    std::thread::spawn(move || {
        loop {
            if let Some(path) = take_open_request(&directory) {
                let _ = sender.send(path);
            }

            std::thread::sleep(WATCH_INTERVAL);
        }
    });
}

pub fn acquire_instance_lock(directory: &Path) -> InstanceLockState {
    let path = directory.join(LOCK_FILE);

    let file = match OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            eprintln!("failed to open instance lock {}: {error}", path.display());
            return InstanceLockState::Unavailable(error);
        }
    };

    match FileExt::try_lock(&file) {
        Ok(()) => InstanceLockState::Acquired(InstanceLock { _file: file }),
        Err(TryLockError::WouldBlock) => InstanceLockState::AlreadyRunning,
        Err(TryLockError::Error(error)) => {
            eprintln!(
                "failed to acquire instance lock {}: {error}",
                path.display()
            );
            InstanceLockState::Unavailable(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InstanceLockState, acquire_instance_lock, first_audio_file_arg, is_supported_audio_path,
        take_open_request, write_open_request,
    };
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "wave_player_lifecycle_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ));
        std::fs::create_dir_all(&directory).expect("create temp lifecycle dir");
        directory
    }

    fn create_audio_file(directory: &PathBuf, name: &str) -> PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, b"audio").expect("create audio fixture");
        path
    }

    #[test]
    fn supported_path_should_check_existence_and_extension_case() {
        let directory = temp_dir("supported");
        let audio = create_audio_file(&directory, "track.MP3");
        let text = create_audio_file(&directory, "notes.txt");

        assert!(is_supported_audio_path(&audio));
        assert!(!is_supported_audio_path(&text));
        assert!(!is_supported_audio_path(&directory.join("missing.wav")));
        assert!(!is_supported_audio_path(&directory));

        std::fs::remove_dir_all(directory).expect("remove temp lifecycle dir");
    }

    #[test]
    fn first_audio_file_arg_should_skip_flags_and_invalid_paths() {
        let directory = temp_dir("args");
        let audio = create_audio_file(&directory, "track.flac");
        let args = vec![
            OsString::from("wave_player.exe"),
            OsString::from("--flag"),
            OsString::from(directory.join("missing.wav")),
            audio.clone().into_os_string(),
            OsString::from("ignored.wav"),
        ];

        assert_eq!(first_audio_file_arg(args), Some(audio));

        std::fs::remove_dir_all(directory).expect("remove temp lifecycle dir");
    }

    #[test]
    fn handoff_should_roundtrip_and_consume_the_request() {
        let directory = temp_dir("handoff");
        let audio = create_audio_file(&directory, "track.wav");

        write_open_request(&directory, &audio).expect("write open request");
        assert_eq!(take_open_request(&directory), Some(audio));
        assert_eq!(take_open_request(&directory), None);

        std::fs::remove_dir_all(directory).expect("remove temp lifecycle dir");
    }

    #[test]
    fn handoff_should_reject_a_missing_file() {
        let directory = temp_dir("handoff-missing");

        write_open_request(&directory, &directory.join("missing.wav")).expect("write request");

        assert_eq!(take_open_request(&directory), None);
        assert!(!directory.join("open_request.json").exists());

        std::fs::remove_dir_all(directory).expect("remove temp lifecycle dir");
    }

    #[test]
    fn instance_lock_should_reject_a_second_owner_until_dropped() {
        let directory = temp_dir("lock");

        let first = acquire_instance_lock(&directory);
        assert!(matches!(first, InstanceLockState::Acquired(_)));

        let second = acquire_instance_lock(&directory);
        assert!(matches!(second, InstanceLockState::AlreadyRunning));

        drop(first);

        let third = acquire_instance_lock(&directory);
        assert!(matches!(third, InstanceLockState::Acquired(_)));

        drop(third);
        std::fs::remove_dir_all(directory).expect("remove temp lifecycle dir");
    }
}
