/* Run:
    pnpm build:css && dx serve --desktop
*/
mod backend;
mod gui;
mod shared;

use crate::gui::route::Route;
use async_broadcast::{Receiver as BroadcastReceiver, broadcast};
use backend::engine_v2::run_audio_loop_v2;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use backend::store::config::ConfigStore;
use shared::store_types::AppConfig;

use backend::store::run::run_store_loop;
use shared::command_types::PlayerCommand;
use shared::command_types::StoreCommand;
use shared::event_types::{AudioError, PlayerEvent, StoreEvent};

use dioxus::desktop::tao::dpi::{LogicalSize, PhysicalPosition, Position};
use dioxus::desktop::tao::window::{Icon, WindowBuilder};
use dioxus::prelude::*;
use generational_box::*;

// fix of FOUC — Flash of Unstyled Content
const TAILWIND_CSS: &str = include_str!("../assets/tailwind.css");

// Shared audio state accessible from any component
#[derive(Clone, Copy)]
pub struct PlayerContext {
    // UI шлет команды в одну сторону через flume
    command_tx: GenerationalBox<flume::Sender<PlayerCommand>, SyncStorage>,

    playback_progress: GenerationalBox<Arc<AtomicU32>, SyncStorage>,
    status_rx: GenerationalBox<BroadcastReceiver<PlayerEvent>, SyncStorage>,
    error_rx: GenerationalBox<BroadcastReceiver<AudioError>, SyncStorage>,

    // Store communication channel
    store_cmd_tx: GenerationalBox<flume::Sender<StoreCommand>, SyncStorage>,
    store_event_rx: GenerationalBox<BroadcastReceiver<StoreEvent>, SyncStorage>,

    // Settings JSON app config
    pub config: GenerationalBox<AppConfig, SyncStorage>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // init app config
    let data_dir = std::env::current_dir().unwrap().join("wave_player_data");
    let config_store = ConfigStore::new(data_dir.join("config.json"));
    let config = config_store.load_with_default();

    // Размер окна из конфига (или дефолт, если не сохранён)
    let x: u32 = config.window.width.unwrap_or(1320);
    let y: u32 = config.window.height.unwrap_or(1040);

    // prepare window icon
    let image = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .expect("failed to load window icon")
        .into_rgba8();

    let icon_width = image.width();
    let icon_height = image.height();

    let window_icon = Icon::from_rgba(image.into_raw(), icon_width, icon_height)
        .expect("failed to create window icon");

    let window = WindowBuilder::new()
        .with_title(format!(
            "Wave Player. Version: {}",
            crate::shared::APP_VERSION
        ))
        .with_window_icon(Some(window_icon))
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

    // init store communication
    let (store_tx, store_rx) = flume::bounded::<StoreCommand>(32);
    let (mut store_broadcast_tx, store_broadcast_rx) = broadcast::<StoreEvent>(12);
    store_broadcast_tx.set_overflow(true);
    run_store_loop(store_rx, store_broadcast_tx, data_dir.clone(), config_store).unwrap();

    // 3. ВКЛЮЧАЕМ OVERFLOW (Защита от лагов интерфейса)
    // Если UI на мобилке или десктопе просел по FPS, старый спам позиций трека
    // сотрется сам, не тормозя аудио-поток
    status_tx.set_overflow(true);
    error_tx.set_overflow(true);

    let store_cmd_tx = store_tx.clone();

    run_audio_loop_v2(
        cmd_rx,
        playback_progress.clone(),
        status_tx,
        store_cmd_tx,
        error_tx,
    )
    .unwrap();

    let owner = SyncStorage::owner();

    // Собираем контекст для UI Dioxus
    let player_ctx = PlayerContext {
        // main channels
        command_tx: owner.insert(cmd_tx),
        playback_progress: owner.insert(playback_progress),
        status_rx: owner.insert(status_rx),
        error_rx: owner.insert(error_rx),

        // store channels
        store_cmd_tx: owner.insert(store_tx),
        store_event_rx: owner.insert(store_broadcast_rx),

        // app's config context
        config: owner.insert(config),
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
                .with_window(window)
                .with_custom_head(format!("<style id=\"app-styles\">{TAILWIND_CSS}</style>")),
        )
        .with_context_provider(move || Box::new(player_ctx))
        .launch(App);

    Ok(())
}

#[component]
fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}
