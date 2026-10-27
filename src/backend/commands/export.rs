/*
- ExportLoop
*/
use crate::backend::offline_render::{RenderMidSideMode, RenderSettings, render_loop};
use crate::backend::state::AudioState;
use crate::shared::event_types::AudioError;
use crate::shared::event_types::PlayerEvent;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::thread;

pub(crate) fn save_loop(state: &mut AudioState, output_path: String) -> Result<(), AudioError> {
    let input_path = state
        .track
        .path
        .clone()
        .ok_or(AudioError::RenderAudioFailed)?;

    let low_pass = cutoff_from_atomic(&state.effects.low_pass);

    let high_pass = cutoff_from_atomic(&state.effects.high_pass);

    let mid_side_mode = match state.effects.mid_side.load(Ordering::Relaxed) {
        1 => RenderMidSideMode::Mid,
        2 => RenderMidSideMode::Side,
        _ => RenderMidSideMode::Stereo,
    };

    let output_path = resolve_output_path(&state.data_dir, &output_path)?;

    let settings = RenderSettings {
        input_path,
        output_path: output_path.clone(),

        loop_start: state.loop_state.start,
        loop_end: state.loop_state.end,

        speed: state.playback.speed,
        preserve_pitch: state.playback.preserve_pitch,

        low_pass,
        high_pass,

        mid_side_mode,

        stereo_width: f32::from_bits(state.effects.stereo_width.load(Ordering::Relaxed)),

        pan: f32::from_bits(state.effects.pan.load(Ordering::Relaxed)),
    };

    let error_tx = state.error_tx.clone();
    let status_tx = state.status_tx.clone();

    thread::Builder::new()
        .name("offline-audio-render".to_string())
        .spawn(move || {
            let _ = status_tx.try_broadcast(PlayerEvent::ExportStarted);

            let _ = status_tx.try_broadcast(PlayerEvent::ExportProgress { progress: 0.05 });

            let progress_status_tx = status_tx.clone();

            let render_result = render_loop(&settings, move |progress| {
                let _ = progress_status_tx.try_broadcast(PlayerEvent::ExportProgress { progress });
            });

            match render_result {
                Ok(()) => {
                    let _ = status_tx.try_broadcast(PlayerEvent::ExportProgress { progress: 1.0 });

                    let _ = status_tx.try_broadcast(PlayerEvent::ExportFinished {
                        path: settings.output_path.clone(),
                    });
                }
                Err(error) => {
                    let _ = status_tx.try_broadcast(PlayerEvent::ExportFailed);

                    let _ = error_tx.try_broadcast(error);
                }
            }
        })
        .map_err(|_| AudioError::RenderAudioFailed)?;

    Ok(())
}

/// Resolves the export destination and makes sure its directory exists.
///
/// Relative paths are resolved against the writable application data
/// directory. `current_dir()` is unreliable here: when launched via
/// `dx serve` it may point at a read-only build directory, which made the
/// WAV writer fail with a generic render error.
fn resolve_output_path(
    data_dir: &std::path::Path,
    output_path: &str,
) -> Result<PathBuf, AudioError> {
    let output_path = PathBuf::from(output_path);

    let output_path = if output_path.is_absolute() {
        output_path
    } else {
        data_dir.join(output_path)
    };

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| AudioError::RenderAudioFailed)?;
    }

    Ok(output_path)
}

fn cutoff_from_atomic(value: &std::sync::Arc<std::sync::atomic::AtomicU32>) -> Option<f32> {
    let cutoff = value.load(Ordering::Relaxed);

    if cutoff == 0 {
        None
    } else {
        Some(cutoff as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_output_path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();

            let path = std::env::temp_dir().join(format!(
                "wave-player-export-{}-{unique}-{}",
                std::process::id(),
                NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
            ));

            std::fs::create_dir_all(&path).unwrap();

            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn relative_path_should_resolve_against_data_dir() {
        let data_dir = TempDir::new();

        let resolved = resolve_output_path(&data_dir.path, "exported_loop.wav").unwrap();

        assert_eq!(resolved, data_dir.path.join("exported_loop.wav"));
    }

    #[test]
    fn relative_path_should_create_missing_parent_directories() {
        let data_dir = TempDir::new();

        let resolved =
            resolve_output_path(&data_dir.path, "exports/loops/exported_loop.wav").unwrap();

        assert_eq!(
            resolved,
            data_dir.path.join("exports/loops/exported_loop.wav")
        );
        assert!(resolved.parent().unwrap().is_dir());
    }

    #[test]
    fn absolute_path_should_be_kept_as_is() {
        let data_dir = TempDir::new();
        let absolute = data_dir.path.join("absolute.wav");

        let resolved = resolve_output_path(&data_dir.path, &absolute.to_string_lossy()).unwrap();

        assert_eq!(resolved, absolute);
    }
}
