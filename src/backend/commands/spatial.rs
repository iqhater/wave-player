/*
- SetMidSideMode
- SetStereoWidth
- SetPan
 */
use std::sync::atomic::Ordering;

use crate::backend::state::AudioState;
use crate::shared::event_types::AudioError;

/// Sets the monitor mode.
///
/// * `0` — stereo
/// * `1` — mid
/// * `2` — side
pub(crate) fn set_mid_side_mode(state: &mut AudioState, mode: u8) -> Result<(), AudioError> {
    let mode = match mode {
        0..=2 => mode,
        _ => 0,
    };

    state.effects.mid_side.store(mode as u32, Ordering::Relaxed);

    Ok(())
}

/// Sets stereo width.
///
/// * `0.0` — mono
/// * `1.0` — original stereo width
/// * `2.0` — maximum supported widening
pub(crate) fn set_stereo_width(state: &mut AudioState, width: f32) -> Result<(), AudioError> {
    let width = width.clamp(0.0, 2.0);

    state
        .effects
        .stereo_width
        .store(width.to_bits(), Ordering::Relaxed);

    Ok(())
}

/// Sets stereo pan.
///
/// * `-1.0` — full left
/// * `0.0` — center
/// * `1.0` — full right
pub(crate) fn set_pan(state: &mut AudioState, pan: f32) -> Result<(), AudioError> {
    let pan = pan.clamp(-1.0, 1.0);

    state.effects.pan.store(pan.to_bits(), Ordering::Relaxed);

    Ok(())
}
