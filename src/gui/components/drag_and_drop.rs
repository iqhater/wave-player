use dioxus::prelude::*;

#[component]
pub fn DragAndDropFile(is_dragover_event: Signal<bool>) -> Element {
    let zone_class = if is_dragover_event() {
        "border-primary bg-primary/10"
    } else {
        "border-outline/60 hover:border-primary/60 hover:bg-surface-variant/20"
    };

    rsx! {
        if is_dragover_event() {
            div {
                class: "absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 select-none cursor-pointer rounded-2xl border-2 border-dashed pointer-events-none transition-all duration-200 {zone_class}",
                span { class: "material-symbols-outlined text-primary/70", style: "font-size: 36px;", "audio_file" }
                span { class: "text-sm font-medium text-primary/90", "Drag & Drop your audio file here" }
                span { class: "text-[11px] text-secondary opacity-80", "mp3, wav, flac, ogg, m4a, aac" }
            }
        }
    }
}
