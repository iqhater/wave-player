use super::{export, filters, loop_control, playback, spatial, track};
use std::path::PathBuf;

use crate::backend::state::AudioState;
use crate::shared::command_types::PlayerCommand;
use crate::shared::event_types::AudioError;
use crate::shared::event_types::PlayerEvent;

pub(crate) fn dispatch(
    state: &mut AudioState,
    player_command: PlayerCommand,
    // store_command: &mut StoreCommand,
) -> Result<(), AudioError> {
    // Player commands
    match player_command {
        // Playback commands
        PlayerCommand::Play(path) => playback::play(state, PathBuf::from(path)),

        PlayerCommand::Pause => playback::pause(state),

        PlayerCommand::Resume => playback::resume(state),

        PlayerCommand::Seek(position) => playback::seek(state, position),

        PlayerCommand::SetSpeed(speed) => playback::set_speed(state, speed),

        PlayerCommand::SetVolume(volume) => playback::set_volume(state, volume),

        PlayerCommand::ToggleRepeatTrack(enabled) => playback::toggle_repeat(state, enabled),

        PlayerCommand::SetPreservePitch(enabled) => playback::set_preserve_pitch(state, enabled),

        // Loop commands
        PlayerCommand::SetLoop(start, end) => loop_control::set_loop(state, start, end),

        PlayerCommand::ToggleLoop(enabled) => loop_control::toggle_loop(state, enabled),

        // Filters commands
        PlayerCommand::SetLowPassFilter(cutoff) => filters::set_low_pass(state, cutoff),

        PlayerCommand::SetHighPassFilter(cutoff) => filters::set_high_pass(state, cutoff),

        // Spatial commands
        PlayerCommand::SetMidSideMode(mode) => spatial::set_mid_side_mode(state, mode),

        PlayerCommand::SetStereoWidth(width) => spatial::set_stereo_width(state, width),

        PlayerCommand::SetPan(pan) => spatial::set_pan(state, pan),

        // Input/Output load & save commands
        PlayerCommand::LoadFile(path) => track::load_file(state, PathBuf::from(path)),

        PlayerCommand::PlayQueue { tracks, current } => {
            playback::play_queue(state, tracks, current)
        }

        PlayerCommand::NextTrack => playback::next_track(state),

        PlayerCommand::PreviousTrack => playback::previous_track(state),

        PlayerCommand::SetShuffle(enabled) => playback::set_shuffle(state, enabled),

        PlayerCommand::ExportLoop(path) => export::save_loop(state, path),

        PlayerCommand::RequestWaveformDetail {
            request_id,
            start,
            end,
            points,
        } => track::request_waveform_detail(state, request_id, start, end, points),

        PlayerCommand::RequestWaveformOverview => track::request_waveform_overview(state),

        // State commands
        PlayerCommand::RequestState => {
            let path = state.track.path.clone();
            let is_playing = playback::is_playing(state);

            let _ = state
                .status_tx
                .try_broadcast(PlayerEvent::PlayerState { path, is_playing });

            state.publish_queue_state();

            Ok(())
        }
    }
}
