/*
- SetLowPassFilter
- SetHighPassFilter
 */
use std::sync::atomic::Ordering;

use crate::backend::state::AudioState;
use crate::shared::event_types::AudioError;

/// Sets or disables the low-pass filter.
///
/// `None` disables the filter.
pub(crate) fn set_low_pass(state: &mut AudioState, cutoff: Option<f32>) -> Result<(), AudioError> {
    let cutoff = cutoff
        .map(|frequency| frequency.clamp(5.0, 24_000.0) as u32)
        .unwrap_or(0);

    state.effects.low_pass.store(cutoff, Ordering::Relaxed);

    Ok(())
}

/// Sets or disables the high-pass filter.
///
/// `None` disables the filter.
pub(crate) fn set_high_pass(state: &mut AudioState, cutoff: Option<f32>) -> Result<(), AudioError> {
    let cutoff = cutoff
        .map(|frequency| frequency.clamp(20.0, 20_500.0) as u32)
        .unwrap_or(0);

    state.effects.high_pass.store(cutoff, Ordering::Relaxed);

    Ok(())
}
