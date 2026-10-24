use super::beat_grid::resolved_grid_tempo;
use crate::PlayerContext;
use crate::gui::layout::{PlaybackUIState, ShowBeatGrid};
use crate::shared::store_types::ManualGrid;
use dioxus::prelude::*;

const MIN_GRID_BPM: f32 = 20.0;
const MAX_GRID_BPM: f32 = 300.0;

fn format_bpm(bpm: f32) -> String {
    format!("{:.0}", bpm.round())
}

fn parse_bpm(text: &str) -> Option<f32> {
    let bpm = text.trim().replace(',', ".").parse::<f32>().ok()?;

    (bpm.is_finite() && (MIN_GRID_BPM..=MAX_GRID_BPM).contains(&bpm)).then_some(bpm)
}

fn apply_bpm_draft(mut draft: Signal<Option<String>>, mut manual: Signal<Option<f32>>) {
    if let Some(text) = draft() {
        if let Some(bpm) = parse_bpm(&text) {
            manual.set(Some(bpm));
        }
    }

    draft.set(None);
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
                            apply_bpm_draft(tempo_draft, grid_tempo);
                        },
                        oninput: move |e| tempo_draft.set(Some(e.value())),
                        onchange: move |_| {
                            apply_bpm_draft(tempo_draft, grid_tempo);
                            persist_current_grid();
                        },
                        onkeydown: move |e| {
                            match e.key() {
                                Key::Enter => {
                                    apply_bpm_draft(tempo_draft, grid_tempo);
                                    persist_current_grid();
                                }
                                Key::Escape => tempo_draft.set(None),
                                _ => {}
                            }
                        },
                        oncontextmenu: move |e| {
                            e.prevent_default();
                            tempo_draft.set(None);
                            grid_tempo.set(None);
                            persist_current_grid();
                        },
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
}
