#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

/* Run:
 *
    Desktop:
    pnpm build:css && dx serve --desktop

    Mobile Android:
    pnpm build:css && dx serve --android
*/
mod backend;
mod gui;
mod shared;
mod updater;

use crate::gui::route::Route;
use crate::updater::{UpdateCommand, UpdateState};
use async_broadcast::{Receiver as BroadcastReceiver, broadcast};
use backend::engine_v2::run_audio_loop_v2;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use backend::store::config::ConfigStore;
use shared::store_types::AppConfig;

use backend::store::run::run_store_loop;
use shared::command_types::PlayerCommand;
use shared::command_types::StoreCommand;
use shared::event_types::{AudioError, PlayerEvent, StoreEvent};

#[cfg(feature = "desktop")]
use dioxus::desktop::tao::dpi::{LogicalSize, PhysicalPosition, Position};

#[cfg(feature = "desktop")]
use dioxus::desktop::tao::window::{Icon, WindowBuilder};
use dioxus::prelude::*;
use generational_box::*;

// fix of FOUC — Flash of Unstyled Content
#[cfg(feature = "desktop")]
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

    // In-app updater
    update_cmd_tx: GenerationalBox<flume::Sender<UpdateCommand>, SyncStorage>,
    update_state_rx: GenerationalBox<BroadcastReceiver<UpdateState>, SyncStorage>,

    // Settings JSON app config
    pub config: GenerationalBox<AppConfig, SyncStorage>,

    // Нужен для синхронной записи config.json при закрытии окна.
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    config_store: GenerationalBox<ConfigStore, SyncStorage>,
}

impl PlayerContext {
    pub(crate) fn update_config(&self, update: impl FnOnce(&mut AppConfig)) {
        let mut config = self.config.read().clone();
        update(&mut config);

        *self.config.write() = config.clone();

        // try_send не блокирует UI, если очередь store временно заполнена.
        let _ = self
            .store_cmd_tx
            .read()
            .try_send(StoreCommand::SetConfig(config));
    }

    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub(crate) fn save_config_now(&self) {
        let config = self.config.read().clone();
        self.config_store.read().save(&config);
    }
}

fn app_data_dir() -> PathBuf {
    // Keep the existing portable layout when the working directory is writable.
    if let Ok(current) = std::env::current_dir() {
        let portable = current.join("wave_player_data");

        if std::fs::create_dir_all(&portable).is_ok() {
            return portable;
        }
    }

    let fallback = platform_data_dir();
    let _ = std::fs::create_dir_all(&fallback);

    fallback
}

fn platform_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA"))
        {
            return PathBuf::from(base).join("Wave Player");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library/Application Support/Wave Player");
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(base) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(base).join("wave-player");
        }

        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local/share/wave-player");
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("wave_player_data")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // init app config
    let data_dir = app_data_dir();
    let config_store = ConfigStore::new(data_dir.join("config.json"));
    let config = config_store.load_with_default();

    #[cfg(feature = "desktop")]
    let window = {
        // Размер окна из конфига (или дефолт, если не сохранён)
        let width: u32 = config.window.width.unwrap_or(1320);
        let height: u32 = config.window.height.unwrap_or(1040);

        let position = match (config.window.x, config.window.y) {
            (Some(x), Some(y)) => Position::new(PhysicalPosition::new(x, y)),
            _ => Position::new(PhysicalPosition::new(
                (3840 - width) / 3,
                (2160 - height) / 10,
            )),
        };

        // prepare window icon
        let image = image::load_from_memory(include_bytes!("../assets/icon.png"))
            .expect("failed to load window icon")
            .into_rgba8();

        let icon_width = image.width();
        let icon_height = image.height();

        let window_icon = Icon::from_rgba(image.into_raw(), icon_width, icon_height)
            .expect("failed to create window icon");

        WindowBuilder::new()
            .with_title(format!(
                "Wave Player. Version: {}",
                crate::shared::version_label()
            ))
            .with_window_icon(Some(window_icon))
            .with_inner_size(LogicalSize::new(width, height))
            .with_position(position)
            .with_maximized(config.window.maximized)
    };

    // 1. Создаем канал связи для команд ОТ UI К audio engine (backend)
    let (cmd_tx, cmd_rx) = flume::bounded::<PlayerCommand>(32);

    // 2. Создаем broadcast каналы для событий ОТ аудио движка К UI.
    let playback_progress = Arc::new(AtomicU32::new(0));
    let (mut status_tx, status_rx) = broadcast::<PlayerEvent>(128);
    let (mut error_tx, error_rx) = broadcast::<AudioError>(10);

    // init store communication
    let (store_tx, store_rx) = flume::bounded::<StoreCommand>(32);
    let context_config_store = config_store.clone();
    let (mut store_broadcast_tx, store_broadcast_rx) = broadcast::<StoreEvent>(12);
    store_broadcast_tx.set_overflow(true);
    run_store_loop(store_rx, store_broadcast_tx, data_dir.clone(), config_store).unwrap();

    // 3. ВКЛЮЧАЕМ OVERFLOW (Защита от лагов интерфейса)
    // Если UI на мобилке или десктопе просел по FPS, старый спам позиций трека
    // сотрется сам, не тормозя аудио-поток
    status_tx.set_overflow(true);
    error_tx.set_overflow(true);

    let store_cmd_tx = store_tx.clone();

    let (update_cmd_tx, update_cmd_rx) = flume::bounded::<UpdateCommand>(8);
    let (mut update_state_tx, update_state_rx) = broadcast::<UpdateState>(32);
    update_state_tx.set_overflow(true);
    updater::spawn(update_cmd_rx, update_state_tx, data_dir.clone());

    run_audio_loop_v2(
        cmd_rx,
        playback_progress.clone(),
        status_tx,
        store_cmd_tx,
        error_tx,
        data_dir.join("cache"),
    )
    .unwrap();

    let owner = SyncStorage::owner();

    // Собираем контекст для UI Dioxus
    #[cfg_attr(
        not(any(feature = "desktop", feature = "mobile")),
        allow(unused_variables)
    )]
    let player_ctx = PlayerContext {
        // main channels
        command_tx: owner.insert(cmd_tx),
        playback_progress: owner.insert(playback_progress),
        status_rx: owner.insert(status_rx),
        error_rx: owner.insert(error_rx),

        // store channels
        store_cmd_tx: owner.insert(store_tx),
        store_event_rx: owner.insert(store_broadcast_rx),

        // in-app updater
        update_cmd_tx: owner.insert(update_cmd_tx),
        update_state_rx: owner.insert(update_state_rx),

        // app's config context
        config: owner.insert(config),

        config_store: owner.insert(context_config_store),
    };

    // Вместо current_dir() используем путь к самому .exe файлу
    // let mut exe_path = std::env::current_exe().unwrap();
    // exe_path.pop(); // Удаляем имя самого файла программы из пути, оставляя только папку
    // let cache_dir = exe_path.join("webview_cache");

    #[cfg(feature = "desktop")]
    {
        let cache_dir = std::env::current_dir()
            .expect("failed to get current directory")
            .join("webview_cache");

        dioxus::LaunchBuilder::new()
            .with_cfg(
                dioxus::desktop::Config::new()
                    .with_data_directory(cache_dir)
                    .with_window(window)
                    .with_custom_head(format!("<style id=\"app-styles\">{TAILWIND_CSS}</style>")),
            )
            .with_context_provider(move || Box::new(player_ctx))
            .launch(App);
    }

    #[cfg(feature = "mobile")]
    {
        dioxus::LaunchBuilder::new()
            /* .with_cfg(
                dioxus::desktop::Config::new()
                    // .with_data_directory(cache_dir)
                    // .with_window(window)
                    .with_custom_head(format!("<style id=\"app-styles\">{TAILWIND_CSS}</style>")),
            ) */
            .with_context_provider(move || Box::new(player_ctx))
            .launch(App);
    }

    /* // 1. Создаем путь к папке "webview_cache" в корне вашего проекта
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
    */
    Ok(())
}

#[cfg(feature = "desktop")]
#[component]
fn PlatformStyles() -> Element {
    rsx! {}
}

#[cfg(not(feature = "desktop"))]
#[component]
fn PlatformStyles() -> Element {
    rsx! {
        document::Stylesheet {
            href: asset!("/assets/tailwind.css"),
        }
    }
}

#[component]
fn App() -> Element {
    rsx! {
        PlatformStyles {}

        Router::<Route> {}
    }
}
