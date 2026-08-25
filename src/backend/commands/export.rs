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
use std::time::Duration;

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

    let output_path = PathBuf::from(output_path);

    let output_path = if output_path.is_absolute() {
        output_path
    } else {
        std::env::current_dir()
            .map_err(|_| AudioError::RenderAudioFailed)?
            .join(output_path)
    };

    let settings = RenderSettings {
        input_path,
        output_path,

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

fn cutoff_from_atomic(value: &std::sync::Arc<std::sync::atomic::AtomicU32>) -> Option<f32> {
    let cutoff = value.load(Ordering::Relaxed);

    if cutoff == 0 {
        None
    } else {
        Some(cutoff as f32)
    }
}
