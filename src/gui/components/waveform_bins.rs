use crate::PlayerContext;
use crate::shared::event_types::PlayerEvent;
use dioxus::prelude::*;
use futures_util::StreamExt;

#[component]
pub fn WaveformBins(total_duration: ReadSignal<f32>, cursor_progress: ReadSignal<f32>) -> Element {
    let ctx = use_context::<PlayerContext>();

    // Фиксированное количество столбиков (бин), которое мы запросили у бэкенда
    let num_bins = 500;

    // Локальный стейт, хранящий высоты всех столбиков волноформы (от 0.0 до 1.0)
    let mut waveform_peaks = use_signal(|| vec![0.0f32; num_bins]);
    let current_cursor = cursor_progress();

    // Фоновый слушатель событий от аудио-ядра
    use_future(move || {
        let mut status_rx = ctx.status_rx.read().clone();
        async move {
            while let Some(status) = status_rx.next().await {
                match status {
                    // Когда загружается новый трек — полностью очищаем старую сетку в ноль
                    PlayerEvent::Started { .. } => {
                        waveform_peaks.set(vec![0.0f32; num_bins]);
                    }

                    // Ловим прилетающие чанки (кусочки по 10 штук) и вмерживаем их в стейт
                    PlayerEvent::WaveformChunkReady {
                        chunk,
                        start_bin,
                        total_bins,
                    } => {
                        // Если разрешение бэкенда не совпадает, подгоняем размер вектора
                        if waveform_peaks.read().len() != total_bins {
                            waveform_peaks.set(vec![0.0f32; total_bins]);
                        }

                        // Безопасное обновление конкретного диапазона индексов
                        waveform_peaks.with_mut(|vec| {
                            for (i, peak_value) in chunk.into_iter().enumerate() {
                                let target_idx = start_bin + i;
                                if let Some(elem) = vec.get_mut(target_idx) {
                                    *elem = peak_value;
                                }
                            }
                        });
                    }
                    _ => {}
                }
            }
        }
    });

    let wave_peaks = waveform_peaks.read();
    let total_len = wave_peaks.len();

    // Динамический отступ между барами в зависимости от плотности
    let gap_px = if total_len <= 100 {
        4.0f32
    } else if total_len <= 300 {
        2.0f32
    } else if total_len <= 600 {
        1.0f32
    } else {
        0.5f32
    };

    rsx! {
        // Контейнер всей волноформы
        div {
            id: "waveform-bins-container",
            // pointer-events-none отклоняет клики по барам, чтобы перематывать корректно
            class: "w-full h-18 flex-shrink-0 flex items-center justify-between pointer-events-none",
            style: "gap: {gap_px}px;",

            // Рендерим каждый столбик на основе данных из вектора peaks
            {wave_peaks.iter().enumerate().map(|(idx, &peak)| {
                let global_pct = idx as f64 / total_len as f64 * 100.0;
                let played = global_pct <= (current_cursor - 0.5) as f64;

                // Вычисляем высоту в процентах. Ограничиваем минимальную высоту в 3%
                let bar_height = (peak * 100.0).max(3.0);

                let bg = if played {
                    "#5a7a7a" // Сыграно
                } else {
                    "#c4c8c0" // Обычный (не сыграно)
                };

                rsx! {
                    div {
                        key: "bar-{idx}",
                        // flex-1 заставляет каждый бар занимать равную долю ширины,
                        // а gap задаёт отступы. Это автоматически подстраивается
                        // под любое количество баров и ширину контейнера.
                        class: "wave-bar rounded-full flex-1",
                        style: "height: {bar_height}%; background: {bg};",
                    }
                }
            })}
        }
    }
}
