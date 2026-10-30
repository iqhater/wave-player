use super::beat_grid::resolved_grid_tempo;
use crate::PlayerContext;
use crate::gui::layout::{PlaybackUIState, ShowBeatGrid};
use crate::shared::command_types::StoreCommand;
use crate::shared::store_types::ManualGrid;
use dioxus::prelude::*;
use std::path::PathBuf;

const MIN_GRID_BPM: f32 = 20.0;
const MAX_GRID_BPM: f32 = 300.0;

fn format_bpm(bpm: f32) -> String {
    let value = rounded_bpm(bpm).unwrap_or(bpm);
    format!("{value:.0}")
}

/// Rounds a BPM to the integer value used both by the display and the library.
pub(crate) fn rounded_bpm(bpm: f32) -> Option<f32> {
    if bpm.is_finite() && (MIN_GRID_BPM..=MAX_GRID_BPM).contains(&bpm) {
        Some(bpm.round())
    } else {
        None
    }
}

fn parse_bpm(text: &str) -> Option<f32> {
    let bpm = text.trim().replace(',', ".").parse::<f32>().ok()?;

    (bpm.is_finite() && (MIN_GRID_BPM..=MAX_GRID_BPM).contains(&bpm)).then_some(bpm)
}

fn persist_track_bpm(ctx: &PlayerContext, track_path: Option<String>, bpm: f32) {
    let Some(path) = track_path else {
        return;
    };
    let Some(bpm) = rounded_bpm(bpm) else {
        return;
    };

    let _ = ctx.store_cmd_tx.read().send(StoreCommand::SetTrackBpm {
        path: PathBuf::from(path),
        bpm,
    });
}

fn commit_bpm(
    ctx: &PlayerContext,
    track_path: Option<String>,
    mut draft: Signal<Option<String>>,
    mut grid_tempo: Signal<Option<f32>>,
    grid_anchor: Signal<Option<f32>>,
) {
    let entered = committed_bpm(draft().as_deref(), grid_tempo());
    draft.set(None);

    let Some(entered) = entered else {
        return;
    };

    grid_tempo.set(Some(entered));
    persist_grid_override(ctx, track_path.clone(), Some(entered), grid_anchor());
    persist_track_bpm(ctx, track_path, entered);
}

/// Returns the parsed value only when it is valid and actually changes the
/// current manual tempo. This keeps blur + change from writing twice.
fn committed_bpm(draft: Option<&str>, current: Option<f32>) -> Option<f32> {
    let entered = draft.and_then(parse_bpm)?;

    (current != Some(entered)).then_some(entered)
}

fn persist_grid_override(
    ctx: &PlayerContext,
    track_path: Option<String>,
    tempo: Option<f32>,
    anchor: Option<f32>,
) {
    let Some(track_path) = track_path else {
        return;
    };

    ctx.update_config(|config| {
        if tempo.is_none() && anchor.is_none() {
            config.ui.manual_grids.remove(&track_path);
        } else {
            config
                .ui
                .manual_grids
                .insert(track_path.clone(), ManualGrid { tempo, anchor });
        }
    });
}

#[component]
pub fn BpmKey() -> Element {
    let ctx = use_context::<PlayerContext>();
    let playback = use_context::<PlaybackUIState>();
    let show_grid = use_context::<ShowBeatGrid>().0;
    let metadata = playback.metadata;

    let bpm = playback.bpm;
    let key = playback.key_name;
    let key_numerical = playback.key_numerical;
    let beat_grid = playback.beat_grid;
    let mut grid_anchor = playback.grid_anchor;
    let mut grid_tempo = playback.grid_tempo;
    let position_secs = playback.position_secs;
    let mut tempo_draft = use_signal(|| None::<String>);
    let mut tempo_focused = use_signal(|| false);

    let persist_current_grid = move || {
        let track_path = metadata
            .read()
            .as_ref()
            .map(|track| track.track_path.clone());

        persist_grid_override(&ctx, track_path, grid_tempo(), grid_anchor());
    };

    let commit_current_bpm = move || {
        let track_path = metadata
            .read()
            .as_ref()
            .map(|track| track.track_path.clone());

        commit_bpm(&ctx, track_path, tempo_draft, grid_tempo, grid_anchor);
    };

    let has_grid = move || show_grid() && bpm() > 0.0 && beat_grid().is_some();
    let is_synced = move || grid_anchor().is_some();

    let tempo_text = move || {
        tempo_draft().unwrap_or_else(|| {
            let tempo = grid_tempo().unwrap_or_else(|| bpm());

            if tempo > 0.0 {
                format_bpm(tempo)
            } else {
                "—".to_string()
            }
        })
    };

    let sync_grid = move |_| {
        if beat_grid().is_some() && resolved_grid_tempo(bpm(), grid_tempo()).is_some() {
            grid_anchor.set(Some(position_secs()));
            persist_current_grid();
        }
    };

    let reset_grid = move |e: MouseEvent| {
        e.prevent_default();
        grid_anchor.set(None);
        persist_current_grid();
    };

    let reset_bpm = move |e: MouseEvent| {
        e.prevent_default();

        let track_path = metadata
            .read()
            .as_ref()
            .map(|track| track.track_path.clone());

        tempo_draft.set(None);
        grid_tempo.set(None);
        persist_grid_override(&ctx, track_path.clone(), None, grid_anchor());

        if bpm() > 0.0 {
            persist_track_bpm(&ctx, track_path, bpm());
        }
    };

    let camelot = move || {
        let numerical = key_numerical();

        if numerical.is_empty() {
            String::new()
        } else {
            format!("({numerical})")
        }
    };

    rsx! {
        div { class: "flex items-start justify-center gap-12 lg:gap-4",
            div { class: "relative flex flex-col",
                if has_grid() {
                    button {
                        class: if is_synced() {
                            "absolute flex h-7 cursor-pointer items-center gap-1 rounded-md bg-primary/15 px-2 text-[12px] font-medium text-primary transition-colors hover:bg-primary/25 active:scale-95"
                        } else {
                            "absolute flex h-7 cursor-pointer items-center gap-1 rounded-md bg-surface-container px-2 text-[12px] font-medium text-secondary transition-colors hover:bg-surface-variant hover:text-primary active:scale-95"
                        },
                        style: "right: 100%; bottom: 0; margin-right: 8px; width: max-content; white-space: nowrap;",
                        title: "Align playhead with a strong or weak beat to sync BPM",
                        onclick: sync_grid,
                        oncontextmenu: reset_grid,
                        span { class: "material-symbols-outlined", style: "font-size: 16px; line-height: 1;", "sync" }
                        span { "Sync grid" }
                    }
                }
                span { class: "text-label-sm uppercase text-secondary opacity-60", "BPM" }
                if bpm() > 0.0 {
                    input {
                        r#type: "text",
                        inputmode: "decimal",
                        value: "{tempo_text()}",
                        class: if tempo_focused() {
                            "w-14 cursor-text rounded-md border border-primary/50 bg-surface-container-low px-1 text-2xl font-bold text-primary outline-none transition-colors lg:text-xl"
                        } else {
                            "w-14 cursor-text rounded-md border border-transparent bg-transparent px-1 text-2xl font-bold text-primary outline-none transition-colors lg:text-xl"
                        },
                        title: "Detected {bpm()} BPM — click to set the exact grid BPM (right-click to reset)",
                        onfocus: move |_| tempo_focused.set(true),
                        onblur: move |_| {
                            tempo_focused.set(false);
                            commit_current_bpm();
                        },
                        oninput: move |e| tempo_draft.set(Some(e.value())),
                        onchange: move |_| commit_current_bpm(),
                        onkeydown: move |e| {
                            match e.key() {
                                Key::Enter => commit_current_bpm(),
                                Key::Escape => tempo_draft.set(None),
                                _ => {}
                            }
                        },
                        oncontextmenu: reset_bpm,
                    }
                } else {
                    span { class: "w-14 border border-transparent px-1 text-2xl font-bold text-primary lg:text-xl", "—" }
                }
            }
            div { class: "flex flex-col",
                span { class: "text-label-sm uppercase text-secondary opacity-60", "Key" }
                span { class: "text-2xl font-bold text-primary lg:text-xl", "{key()} {camelot()}" }
            }
        }
    }
}

#[cfg(test)]
mod bpm_input_tests {
    use super::parse_bpm;

    #[test]
    fn format_bpm_should_round_for_display() {
        assert_eq!(super::format_bpm(119.7), "120");
        assert_eq!(super::format_bpm(120.0), "120");
        assert_eq!(super::format_bpm(120.5), "121");
    }

    #[test]
    fn parse_bpm_should_accept_whole_and_fractional_values() {
        assert_eq!(parse_bpm("120"), Some(120.0));
        assert_eq!(parse_bpm(" 127.4 "), Some(127.4));
        assert_eq!(parse_bpm("127,4"), Some(127.4));
    }

    #[test]
    fn parse_bpm_should_reject_invalid_values() {
        assert_eq!(parse_bpm(""), None);
        assert_eq!(parse_bpm("fast"), None);
        assert_eq!(parse_bpm("10"), None);
        assert_eq!(parse_bpm("400"), None);
    }

    #[test]
    fn rounded_bpm_should_match_the_display_rounding() {
        assert_eq!(super::rounded_bpm(127.4), Some(127.0));
        assert_eq!(super::rounded_bpm(127.6), Some(128.0));
        assert_eq!(super::rounded_bpm(20.0), Some(20.0));
        assert_eq!(super::rounded_bpm(300.0), Some(300.0));
    }

    #[test]
    fn rounded_bpm_should_reject_out_of_range_and_non_finite_values() {
        assert_eq!(super::rounded_bpm(19.9), None);
        assert_eq!(super::rounded_bpm(300.1), None);
        assert_eq!(super::rounded_bpm(f32::NAN), None);
        assert_eq!(super::rounded_bpm(f32::INFINITY), None);
    }

    #[test]
    fn committed_bpm_should_only_accept_a_changed_valid_value() {
        assert_eq!(super::committed_bpm(Some("127.4"), None), Some(127.4));
        assert_eq!(super::committed_bpm(Some("127.4"), Some(127.4)), None);
        assert_eq!(super::committed_bpm(Some("fast"), Some(120.0)), None);
        assert_eq!(super::committed_bpm(None, Some(120.0)), None);
    }
}
