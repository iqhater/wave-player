pub(crate) fn dispatch(
    runtime: &mut AudioRuntime,
    command: PlayerCommand,
) -> Result<(), AudioError> {
    match command {
        PlayerCommand::LoadFile(path) => load_file::execute(runtime, path),

        PlayerCommand::Pause => playback::pause(runtime),

        PlayerCommand::Resume => playback::resume(runtime),

        PlayerCommand::Seek(position) => seek::execute(runtime, position),

        PlayerCommand::SetLoop(start, end) => loop_control::set_loop(runtime, start, end),

        PlayerCommand::ToggleLoop(enabled) => loop_control::toggle(runtime, enabled),

        PlayerCommand::SetSpeed(speed) => speed::set_speed(runtime, speed),

        PlayerCommand::SetVolume(volume) => volume::set_volume(runtime, volume),

        PlayerCommand::SetLowPassFilter(cutoff) => filters::set_low_pass(runtime, cutoff),

        PlayerCommand::SetHighPassFilter(cutoff) => filters::set_high_pass(runtime, cutoff),

        PlayerCommand::SetMidSideMode(mode) => spatial::set_mid_side_mode(runtime, mode),

        PlayerCommand::SetStereoWidth(width) => spatial::set_stereo_width(runtime, width),

        PlayerCommand::SetPan(pan) => spatial::set_pan(runtime, pan),

        PlayerCommand::ToggleRepeatTrack(enabled) => repeat::toggle(runtime, enabled),

        PlayerCommand::ExportLoop(path) => export_loop::execute(runtime, path),

        PlayerCommand::Play(path) => load_file::play(runtime, path),
    }
}
