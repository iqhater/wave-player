use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::thread;
use std::time::Duration;

use rodio::{DeviceSinkBuilder, Player};

use crate::backend::commands::{dispatch, playback};
use crate::backend::state::AudioState;
use crate::shared::command_types::{PlayerCommand, StoreCommand};
use crate::shared::event_types::{AudioError, ErrorSender, StatusSender};

/// Runs the second-generation audio engine.
///
/// The engine owns the runtime loop and delegates individual commands
/// to the command modules.
pub fn run_audio_loop_v2(
    cmd_rx: flume::Receiver<PlayerCommand>,
    playback_progress: Arc<AtomicU32>,
    status_tx: StatusSender,
    store_cmd_tx: flume::Sender<StoreCommand>,
    error_tx: ErrorSender,
    cover_cache_dir: PathBuf,
) -> Result<(), Box<dyn Error>> {
    thread::spawn(move || {
        let sink_handle = match DeviceSinkBuilder::open_default_sink() {
            Ok(handle) => handle,
            Err(_) => {
                let _ = error_tx.try_broadcast(AudioError::DeviceInitializationFailed);
                return;
            }
        };

        let player = Player::connect_new(&sink_handle.mixer());

        let mut state = AudioState::new(
            player,
            playback_progress,
            status_tx,
            store_cmd_tx,
            error_tx,
            cover_cache_dir,
        );

        loop {
            if let Err(error) = playback::tick(&mut state) {
                let _ = state.error_tx.try_broadcast(error);
            }

            let is_playing = playback::is_playing(&state);

            let command = if is_playing {
                cmd_rx.try_recv().ok()
            } else {
                match cmd_rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            };

            if let Some(command) = command {
                if let Err(error) = dispatch::dispatch(&mut state, command) {
                    let _ = state.error_tx.try_broadcast(error);
                }
            }

            if is_playing {
                thread::sleep(Duration::from_millis(10));
            }
        }
    });

    Ok(())
}
