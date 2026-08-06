/* Run:
    pnpm build:css && dx serve --desktop
*/
mod backend;
mod gui;
mod shared;

use crate::gui::route::Route;
use async_broadcast::{Receiver as BroadcastReceiver, broadcast};
use backend::engine::run_audio_loop;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use shared::command_types::PlayerCommand;
use shared::event_types::{AudioError, PlayerEvent};

use dioxus::desktop::tao::dpi::{LogicalSize, PhysicalPosition, Position};
use dioxus::desktop::tao::window::WindowBuilder;
use dioxus::prelude::*;
use generational_box::*;

// Shared audio state accessible from any component
#[derive(Clone, Copy)]
pub struct PlayerContext {
    // UI шлет команды в одну сторону через flume
    command_tx: GenerationalBox<flume::Sender<PlayerCommand>, SyncStorage>,

    playback_progress: GenerationalBox<Arc<AtomicU32>, SyncStorage>,
    status_rx: GenerationalBox<BroadcastReceiver<PlayerEvent>, SyncStorage>,
    error_rx: GenerationalBox<BroadcastReceiver<AudioError>, SyncStorage>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let x: u32 = 1320;
    let y: u32 = 1040;

    let window = WindowBuilder::new()
        .with_title("Wave Player. Version: 0.1.0")
        .with_inner_size(LogicalSize::new(x, y))
        .with_position(Position::new(PhysicalPosition::new(
            (3840 - x) / 3,
            (2160 - y) / 10,
        )));

    // 1. Создаем канал связи для команд ОТ UI К audio engine (backend)
    let (cmd_tx, cmd_rx) = flume::bounded::<PlayerCommand>(32);

    // 2. Создаем broadcast каналы для событий ОТ аудио движка К UI.
    let playback_progress = Arc::new(AtomicU32::new(0));
    let (mut status_tx, status_rx) = broadcast::<PlayerEvent>(24);
    let (mut error_tx, error_rx) = broadcast::<AudioError>(10);

    // 3. ВКЛЮЧАЕМ OVERFLOW (Защита от лагов интерфейса)
    // Если UI на мобилке или десктопе просел по FPS, старый спам позиций трека
    // сотрется сам, не тормозя аудио-поток
    status_tx.set_overflow(true);
    error_tx.set_overflow(true);

    run_audio_loop(cmd_rx, playback_progress.clone(), status_tx, error_tx).unwrap();

    let owner = SyncStorage::owner();
    // Собираем контекст для UI Dioxus
    let player_ctx = PlayerContext {
        command_tx: owner.insert(cmd_tx),
        playback_progress: owner.insert(playback_progress),
        status_rx: owner.insert(status_rx),
        error_rx: owner.insert(error_rx),
    };

    // Вместо current_dir() используем путь к самому .exe файлу
    // let mut exe_path = std::env::current_exe().unwrap();
    // exe_path.pop(); // Удаляем имя самого файла программы из пути, оставляя только папку
    // let cache_dir = exe_path.join("webview_cache");

    // 1. Создаем путь к папке "webview_cache" в корне вашего проекта
    let cache_dir = std::env::current_dir().unwrap().join("webview_cache");
    // let desktop_config = dioxus::desktop::Config::default().with_data_directory(cache_dir);

    // let custom_css = include_str!("../assets/fonts/Geist[wght].woff2");

    dioxus::LaunchBuilder::new()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_data_directory(cache_dir) // 2. Жестко привязываем WebView2 к этой папке
                .with_window(window),
            // .with_custom_head(format!("<style>{}</style>", custom_css)),
        )
        .with_context_provider(move || Box::new(player_ctx))
        .launch(App);

    Ok(())
}

#[component]
fn App() -> Element {
    let ctx = use_context::<PlayerContext>();

    // Load default track on startup
    use_effect(move || {
        spawn(async move {
            let path = PathBuf::from("D:/Web_Dev/Dev/Rust/wave_player/audio_examples")
                /* let path = PathBuf::from(
                    "F:/Downloads/Déepalma Ibiza 2026 FLAC/Déepalma Ibiza 2026 FLAC",
                ) */
                .join("ENG_US_M_PaulQ.mp3");
            /* .join(
                "49.  Yves Murasca & Rosario Galati  -  Déepalma Ibiza 2026 (Beach Feelings).flac",
            ); */
            let _ = ctx
                .command_tx
                .read()
                .send(PlayerCommand::LoadFile(path.to_string_lossy().to_string()));
        });
    });

    rsx! {
        Router::<Route> {}
    }
}
