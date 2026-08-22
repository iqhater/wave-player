/*
- ExportLoop
*/
use crate::backend::state::AudioState;
use crate::shared::event_types::AudioError;

// TODO: implement offline rendering
pub(crate) fn save_loop(_state: &mut AudioState, _path: String) -> Result<(), AudioError> {
    Err(AudioError::RenderAudioFailed)
}
