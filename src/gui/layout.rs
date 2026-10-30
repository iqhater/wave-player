use super::route::Route;
use crate::shared::command_types::{PlayerCommand, StoreCommand};
use dioxus::prelude::*;
use std::path::PathBuf;

use super::components::{
    errors_toast::ErrorsToast, footer::Footer, player_controls::PlayerControls, sidebar::Sidebar,
    theme_toggle::ThemeToggle,
};
use crate::backend::metadata::TrackMetadata;

// use super::screens::home::Home;
use super::theme::{Theme, use_theme, use_theme_provider};
use crate::PlayerContext;
use crate::shared::event_types::{BeatGridData, PlayerEvent, StoreEvent};
use crate::updater::UpdateState;
use futures_util::StreamExt;

#[cfg(feature = "desktop")]
use dioxus::desktop::tao::dpi::{LogicalSize, PhysicalPosition};
#[cfg(feature = "desktop")]
use dioxus::desktop::tao::event::Event as TaoEvent;
#[cfg(feature = "desktop")]
use dioxus::desktop::{WindowEvent, use_window, use_wry_event_handler};

// Geist link "https://fonts.googleapis.com/css2?family=Geist:wght@100;300;400;500;600;700;800;900&display=swap"
static FAVICON: Asset = asset!("/assets/icon.png");
// static TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");
// static GOOGLE_FONTS_GEIST_LINK: &str = "https://fonts.googleapis.com/css2?family=Geist:wght@100;300;400;500;600;700;800;900&display=swap";
// static MATERIAL_SYMBOLS_OUTLINED_LINK: &str = "https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:wght,FILL@100..700,0..1&display=swap";

static FONTS_GEIST: Asset = asset!("/assets/fonts/Geist[wght].woff2");
static MATERIAL_SYMBOLS_OUTLINED: Asset =
    asset!("/assets/fonts/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].woff2");
// static CSS: Asset = asset!("/assets/main.css");

// TODO: collapse into one context?
#[derive(Clone, Copy)]
pub struct MobileMenuOpen(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct TimelineCursorFollowsLoop(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct ShowTimeMarkers(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct LoopRepeatRegion(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct TimelineView(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct ShowHoverCursor(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct SidebarCollapsed(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct LoopDragging(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct ShowBeatGrid(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct BeatGridSnap(pub Signal<bool>);

#[derive(Clone, Copy)]
pub struct UpdateUIState {
    pub state: Signal<UpdateState>,
    pub dismissed_tag: Signal<Option<String>>,
}

#[derive(Clone, Copy)]
pub struct PlaybackUIState {
    pub position_secs: Signal<f32>,
    pub duration_secs: Signal<f32>,
    pub has_track: Signal<bool>,
    pub is_playing: Signal<bool>,
    pub speed: Signal<f32>,
    pub preserve_pitch: Signal<bool>,

    pub metadata: Signal<Option<TrackMetadata>>,

    pub(crate) bpm: Signal<f32>,
    pub(crate) key_name: Signal<String>,
    pub(crate) key_numerical: Signal<String>,
    pub(crate) beat_grid: Signal<Option<BeatGridData>>,
    /// Time that must stay on a beat line after manual grid sync.
    pub(crate) grid_anchor: Signal<Option<f32>>,
    /// Manual grid tempo override; `None` means the analysis tempo is used.
    pub(crate) grid_tempo: Signal<Option<f32>>,
}

#[derive(Clone, Copy)]
pub struct LoopUIState {
    pub start: Signal<f32>,
    pub end: Signal<f32>,
    pub enabled: Signal<bool>,
}

#[cfg(feature = "desktop")]
fn persist_window_state(
    ctx: &PlayerContext,
    size: Option<LogicalSize<u32>>,
    position: Option<PhysicalPosition<i32>>,
    maximized: bool,
) {
    ctx.update_config(|config| {
        // При maximized не перезаписываем нормальный размер окна.
        if let Some(size) = size {
            if !maximized {
                config.window.width = Some(size.width);
                config.window.height = Some(size.height);
            }
        }

        if let Some(position) = position {
            config.window.x = Some(position.x);
            config.window.y = Some(position.y);
        }

        config.window.maximized = maximized;
    });
}

#[component]
pub fn RootLayout() -> Element {
    let ctx = use_context::<PlayerContext>();
    let route = use_route::<Route>();

    let mut mobile_menu_open = use_signal(|| false);
    use_context_provider(|| MobileMenuOpen(mobile_menu_open));

    // Settings config
    let mut cursor_follows_loop = use_signal(|| ctx.config.read().ui.timeline_cursor_follows_loop);
    let mut show_time_markers = use_signal(|| ctx.config.read().ui.show_time_markers);
    let mut loop_repeat_region = use_signal(|| ctx.config.read().ui.loop_repeat_region);
    let mut timeline_progress_view = use_signal(|| ctx.config.read().ui.timeline_progress_view);
    let mut show_beat_grid = use_signal(|| ctx.config.read().ui.show_beat_grid);
    let mut beat_grid_snap = use_signal(|| ctx.config.read().ui.beat_grid_snap);
    let mut show_hover_cursor = use_signal(|| ctx.config.read().ui.show_hover_cursor);
    let mut sidebar_collapsed = use_signal(|| ctx.config.read().ui.sidebar_collapsed);
    let loop_dragging = use_signal(|| false);

    use_context_provider(|| TimelineCursorFollowsLoop(cursor_follows_loop));
    use_context_provider(|| ShowTimeMarkers(show_time_markers));
    use_context_provider(|| LoopRepeatRegion(loop_repeat_region));
    use_context_provider(|| TimelineView(timeline_progress_view));
    use_context_provider(|| ShowBeatGrid(show_beat_grid));
    use_context_provider(|| BeatGridSnap(beat_grid_snap));
    use_context_provider(|| ShowHoverCursor(show_hover_cursor));
    use_context_provider(|| SidebarCollapsed(sidebar_collapsed));
    use_context_provider(|| LoopDragging(loop_dragging));

    // Эти настройки доступны всем дочерним компонентам, включая Settings и Waveform.
    use_context_provider(|| show_beat_grid);
    use_context_provider(|| beat_grid_snap);

    let update_state = use_signal(|| UpdateState::Idle);
    let dismissed_update_tag = use_signal(|| None::<String>);
    use_context_provider(|| UpdateUIState {
        state: update_state,
        dismissed_tag: dismissed_update_tag,
    });

    #[cfg(feature = "desktop")]
    {
        let window = use_window();

        let mut last_save: Option<std::time::Instant> = None;
        let mut last_size: Option<LogicalSize<u32>> = None;
        let mut last_position: Option<PhysicalPosition<i32>> = None;

        use_wry_event_handler(move |event, _| {
            let TaoEvent::WindowEvent { event, .. } = event else {
                return;
            };

            match event {
                WindowEvent::Resized(size) => {
                    last_size = Some(size.to_logical::<u32>(window.scale_factor()));
                }
                WindowEvent::Moved(position) => {
                    last_position = Some(*position);
                }
                WindowEvent::CloseRequested => {
                    persist_window_state(&ctx, last_size, last_position, window.is_maximized());

                    // Дописываем config.json синхронно, до завершения процесса.
                    ctx.save_config_now();
                    return;
                }
                _ => return,
            }

            let should_save = last_save
                .is_none_or(|time| time.elapsed() >= std::time::Duration::from_millis(750));

            if should_save {
                last_save = Some(std::time::Instant::now());

                persist_window_state(&ctx, last_size, last_position, window.is_maximized());
            }
        });
    }

    let hide_controls = matches!(
        route,
        Route::TermsOfService | Route::PrivacyPolicy | Route::Changelog | Route::AppInfo
    );

    // loop and timeline cursor state
    let position_secs = use_signal(|| 0.0_f32);
    let duration_secs = use_signal(|| 0.0_f32);
    let has_track = use_signal(|| false);
    let is_playing = use_signal(|| false);

    let metadata = use_signal(|| None::<TrackMetadata>);

    let bpm = use_signal(|| 0.0_f32);
    let key_name = use_signal(|| "—".to_string());
    let key_numerical = use_signal(|| String::new());
    let beat_grid = use_signal(|| None::<BeatGridData>);
    let grid_anchor = use_signal(|| None::<f32>);
    let grid_tempo = use_signal(|| None::<f32>);

    let loop_start = use_signal(|| 0.0_f32);
    let loop_end = use_signal(|| 0.0_f32);
    let loop_enabled = use_signal(|| false);

    let mut speed = use_signal(|| ctx.config.read().audio.speed as f32);
    let mut preserve_pitch = use_signal(|| ctx.config.read().audio.preserve_pitch);

    use_context_provider(|| PlaybackUIState {
        position_secs,
        duration_secs,
        has_track,
        is_playing,
        speed,
        preserve_pitch,
        metadata,
        bpm,
        key_name,
        key_numerical,
        beat_grid,
        grid_anchor,
        grid_tempo,
    });

    use_context_provider(|| LoopUIState {
        start: loop_start,
        end: loop_end,
        enabled: loop_enabled,
    });

    // Применяем сохранённые аудионастройки к движку после монтирования UI.
    use_effect(move || {
        let config = ctx.config.read().clone();

        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetVolume(config.audio.volume));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetSpeed(config.audio.speed));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetPreamp(config.audio.preamp_db));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetPreservePitch(config.audio.preserve_pitch));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::ToggleRepeatTrack(config.audio.repeat_track));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetLowPassFilter(config.audio.low_pass));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetHighPassFilter(config.audio.high_pass));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetMidSideMode(config.audio.mid_side));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetStereoWidth(config.audio.stereo_width));
        let _ = ctx
            .command_tx
            .read()
            .send(PlayerCommand::SetPan(config.audio.pan));
    });

    // 1. СРАЗУ читаем сохраненную тему из конфига при старте приложения
    let saved_theme = Theme::from_pref(ctx.config.read().theme.clone());

    use_theme_provider(saved_theme);

    let theme = use_theme();
    let body_class = if theme.get() == Theme::Dark {
        "dark bg-surface text-on-surface"
    } else {
        "bg-surface text-on-surface"
    };

    /* let is_home_route = matches!(route, Route::Home);

    let home_view_class = if is_home_route {
        "w-full max-w-[600px] flex flex-col items-center gap-8"
    } else {
        "hidden"
    }; */

    // Слушаем рантайм-обновления от бэкенда
    use_future(move || {
        let mut store_event_rx = ctx.store_event_rx.read().clone();

        async move {
            while let Some(store_event) = store_event_rx.next().await {
                match store_event {
                    StoreEvent::ConfigLoaded { config } => {
                        *ctx.config.write() = config.clone();
                        theme.set(Theme::from_pref(config.theme.clone()));

                        cursor_follows_loop.set(config.ui.timeline_cursor_follows_loop);
                        show_time_markers.set(config.ui.show_time_markers);
                        loop_repeat_region.set(config.ui.loop_repeat_region);
                        timeline_progress_view.set(config.ui.timeline_progress_view);
                        show_beat_grid.set(config.ui.show_beat_grid);
                        beat_grid_snap.set(config.ui.beat_grid_snap);
                        show_hover_cursor.set(config.ui.show_hover_cursor);
                        sidebar_collapsed.set(config.ui.sidebar_collapsed);

                        // Синхронизируем UI-состояние аудионастроек с конфигурацией.
                        speed.set(config.audio.speed.clamp(0.25, 2.0));
                        preserve_pitch.set(config.audio.preserve_pitch);
                    }

                    _ => {}
                }
            }
        }
    });

    use_future(move || {
        let mut update_rx = ctx.update_state_rx.read().clone();
        let mut update_state = update_state;

        if crate::updater::is_supported() {
            let _ = ctx
                .update_cmd_tx
                .read()
                .try_send(crate::updater::UpdateCommand::Check);
        }

        async move {
            while let Some(state) = update_rx.next().await {
                update_state.set(state);
            }
        }
    });

    // Hooks must run on the component render, not inside `use_future`.
    #[cfg(feature = "desktop")]
    let open_navigator = use_navigator();
    #[cfg(feature = "desktop")]
    let open_window = use_window();

    #[cfg(feature = "desktop")]
    use_future(move || {
        let open_rx = ctx.open_request_rx.read().clone();
        let navigator = open_navigator;
        let window = open_window.clone();
        let command_tx = ctx.command_tx;

        async move {
            while let Ok(path) = open_rx.recv_async().await {
                if !crate::desktop_lifecycle::is_supported_audio_path(&path) {
                    continue;
                }

                navigator.push(Route::Home);

                let _ = command_tx
                    .read()
                    .send(PlayerCommand::LoadFile(path.to_string_lossy().into_owned()));

                window.set_visible(true);
                window.set_minimized(false);
                window.set_focus();
            }
        }
    });

    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();

        let mut position_secs = position_secs;
        let mut duration_secs = duration_secs;
        let mut has_track = has_track;
        let mut is_playing = is_playing;
        let mut metadata = metadata;

        let mut bpm = bpm;
        let mut key_name = key_name;
        let mut key_numerical = key_numerical;
        let mut beat_grid = beat_grid;
        let mut grid_anchor = grid_anchor;
        let mut grid_tempo = grid_tempo;

        async move {
            while let Some(event) = status_rx.next().await {
                match event {
                    PlayerEvent::Started {
                        duration,
                        metadata: track_metadata,
                    } => {
                        let track_path = track_metadata.track_path.clone();
                        let saved_grid =
                            ctx.config.read().ui.manual_grids.get(&track_path).cloned();

                        position_secs.set(0.0);
                        duration_secs.set(duration.as_secs_f32());
                        has_track.set(true);
                        is_playing.set(true);
                        metadata.set(Some(track_metadata));

                        bpm.set(0.0);
                        key_name.set("—".to_string());
                        key_numerical.set(String::new());
                        beat_grid.set(None);
                        grid_anchor.set(saved_grid.as_ref().and_then(|grid| grid.anchor));
                        grid_tempo.set(saved_grid.as_ref().and_then(|grid| grid.tempo));

                        if let Some(tempo) = saved_grid.as_ref().and_then(|grid| grid.tempo) {
                            if let Some(bpm) = crate::gui::components::bpm_key::rounded_bpm(tempo) {
                                let _ = ctx.store_cmd_tx.read().send(StoreCommand::SetTrackBpm {
                                    path: PathBuf::from(&track_path),
                                    bpm,
                                });
                            }
                        }
                    }

                    PlayerEvent::Paused => {
                        is_playing.set(false);
                    }

                    PlayerEvent::Resumed => {
                        is_playing.set(true);
                    }

                    PlayerEvent::Finished => {
                        is_playing.set(false);
                        position_secs.set(0.0);
                    }

                    PlayerEvent::AnalysisCleared => {
                        bpm.set(0.0);
                        key_name.set("—".to_string());
                        key_numerical.set(String::new());
                        beat_grid.set(None);
                    }

                    PlayerEvent::BpmKeyDetected {
                        bpm: detected_bpm,
                        beat_grid: detected_grid,
                        key_name: detected_key,
                        key_numerical: detected_numerical,
                    } => {
                        bpm.set(detected_bpm);
                        key_name.set(detected_key);
                        key_numerical.set(detected_numerical);
                        beat_grid.set(Some(detected_grid));
                    }

                    _ => {}
                }
            }
        }
    });

    let content_width_class = if matches!(route, Route::Favorites | Route::RecentTracks) {
        "w-full max-w-[1250px]"
    } else {
        "w-full max-w-[600px]"
    };

    rsx! {

        document::Link { rel: "icon", href: FAVICON }

        // Approach 2 — register a bundled font with an inline @font-face rule.
        // `{FONTS_GEIST}` interpolates to the asset's resolved URL at runtime.
        style { "@font-face {{ font-family: 'Geist'; src: url('{FONTS_GEIST}') format('woff2'); font-display: swap; }}" }
        style { "@font-face {{ font-family: 'Material Symbols Outlined'; src: url('{MATERIAL_SYMBOLS_OUTLINED}') format('woff2'); font-display: block; }}" }

        // document::Stylesheet { href: TAILWIND_CSS }


        // Custom main.css no needed as tailwind
        // document::Stylesheet { href: CSS }
        // document::Stylesheet {  href: MATERIAL_SYMBOLS_OUTLINED }

        // Container for the entire app
        body { class: "{body_class}",
            div { class: "flex h-screen w-full overflow-hidden relative",

                ThemeToggle {}

                // Mobile top bar
                MobileHeader {}

                // Dark backdrop when the mobile menu is open
                if mobile_menu_open() {
                    div {
                        class: "fixed inset-0 z-40 bg-black/40 lg:hidden",
                        onclick: move |_| mobile_menu_open.set(false),
                    }
                }

                // Sidebar remains unchanged on desktop and becomes an overlay menu
                // on mobile/tablet.
                Sidebar {}

                main { class: "relative flex min-w-0 flex-1 flex-col
                               items-center overflow-hidden bg-surface",
                        div { class: "flex min-h-0 w-full flex-1 flex-col items-center
                                     overflow-y-auto
                                     px-4 pb-52 pt-24
                                     lg:p-container-padding
                                     lg:pb-40",
                           div { class: "{content_width_class}
                                         my-auto flex w-full flex-col items-center gap-6
                                         lg:gap-8",
                               // Router paste here: Home, Recents, Favorites or Settings
                               Outlet::<Route> {}
                           }

                           // All errors overlay notification from audio engine
                           ErrorsToast {}
                    }

                    if !hide_controls {
                        div {
                            class: "pointer-events-none absolute inset-x-0 bottom-0
                                    z-40 flex justify-center lg:bottom-24",
                            div {
                                class: "pointer-events-auto",
                                PlayerControls {}
                            }
                        }
                    }
                }
            }

            // Footer (always seen)
            Footer {}
        }
    }
}

#[component]
fn MobileHeader() -> Element {
    let mut mobile_menu_open = use_context::<MobileMenuOpen>().0;

    rsx! {
        header {
            class: "fixed left-0 right-0 top-0 z-30
                    flex h-20 items-center justify-between
                    bg-surface px-4
                    lg:hidden",

            button {
                class: "flex h-10 w-10 items-center justify-center
                        rounded-xl text-secondary
                        hover:bg-surface-container-low
                        hover:text-primary
                        cursor-pointer transition-all",

                title: "Open menu",
                "aria-label": "Open menu",

                onclick: move |_| mobile_menu_open.set(true),

                span {
                    class: "material-symbols-outlined text-[24px]",
                    "menu"
                }
            }

            div {
                class: "flex flex-col items-center justify-center",

                span {
                    class: "text-[12px] font-bold uppercase
                            tracking-[0.18em] text-secondary",

                    "Audio Space"
                }

                span {
                    class: "text-xl font-bold tracking-tight text-primary",

                    "Wave Player"
                }
            }

            // Empty element keeps the title centered.
            div {
                class: "h-10 w-10",
            }
        }
    }
}
