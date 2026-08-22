/*
- SetLoop
- ToggleLoop
 */
use std::time::Duration;

use crate::backend::state::AudioState;
use crate::shared::event_types::{AudioError, PlayerEvent};

/// Sets the loop boundaries.
pub(crate) fn set_loop(
    state: &mut AudioState,
    start: Duration,
    end: Duration,
) -> Result<(), AudioError> {
    let start = start.min(state.track.duration);
    let end = end.min(state.track.duration);

    state.loop_state.start = start;
    state.loop_state.end = end;

    let _ = state
        .status_tx
        .try_broadcast(PlayerEvent::LoopSet { start, end });

    Ok(())
}

/// Enables or disables the current loop.
pub(crate) fn toggle_loop(state: &mut AudioState, enabled: bool) -> Result<(), AudioError> {
    state.loop_state.enabled = enabled;

    let _ = state
        .status_tx
        .try_broadcast(PlayerEvent::LoopEnabled { is_loop: enabled });

    Ok(())
}
