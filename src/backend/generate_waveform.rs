use crate::shared::event_types::PlayerEvent;
use rodio::decoder::DecoderBuilder;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Duration;

/// Запускает фоновый поток для пошагового расчета волноформы методом итератора.
/// Данные отправляются в UI чанками слева направо.
pub fn stream_waveform<T>(
    path: PathBuf,
    channels: u16,
    sample_rate: u32,
    duration: Duration,
    num_bins: usize,
    status_tx: T, // Передайте сюда ваш тип канала, например BroadcastSender<PlayerEvent>
) where
    T: Clone + Send + 'static,
    // Укажите ваш метод отправки, например:
    T: Fn(PlayerEvent) -> Result<(), Box<dyn std::error::Error>> + Send + Sync,
{
    let channels_count = channels as usize;

    std::thread::spawn(move || {
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return,
        };
        let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        // Создаем второй декодер исключительно для пошагового чтения с диска
        let mut bg_decoder = match DecoderBuilder::new()
            .with_data(BufReader::new(file))
            .with_byte_len(len)
            .with_hint("mp3") // most popular?
            .build()
        {
            Ok(d) => d,
            Err(_) => return,
        };

        let duration_secs = duration.as_secs_f64();
        let total_frames = (duration_secs * sample_rate as f64) as usize;
        if total_frames == 0 {
            return;
        }

        // Сколько аудио-фреймов приходится на один столбик (бин) волноформы
        let frames_per_bin = (total_frames as f64 / num_bins as f64) as usize;

        // Накапливаем по 10 бинов перед отправкой, чтобы не спамить в канал слишком часто
        let bins_per_chunk = 10;
        let mut chunk_data = Vec::with_capacity(bins_per_chunk);
        let mut current_chunk_start_bin = 0;

        for bin in 0..num_bins {
            let mut max_mono_peak = 0.0f32;

            // Вычитываем ровно столько фреймов, сколько нужно для одного столбика
            for _ in 0..frames_per_bin {
                let mut frame_sum = 0.0f32;
                let mut samples_read = 0;

                // Вычитываем сэмплы текущего фрейма (для всех каналов)
                for _ in 0..channels_count {
                    if let Some(sample) = bg_decoder.next() {
                        frame_sum += sample.abs();
                        samples_read += 1;
                    } else {
                        break;
                    }
                }

                if samples_read == 0 {
                    break;
                }

                let mono_frame_val = frame_sum / samples_read as f32;
                if mono_frame_val > max_mono_peak {
                    max_mono_peak = mono_frame_val;
                }
            }

            chunk_data.push(max_mono_peak);

            // Если чанк заполнен или это последний элемент — отправляем в UI
            if chunk_data.len() == bins_per_chunk || bin == num_bins - 1 {
                // Вызываем отправку события. Подгоните под ваш метод (.try_broadcast или .send)
                let _ = status_tx(PlayerEvent::WaveformChunkReady {
                    start_bin: current_chunk_start_bin,
                    total_bins: num_bins,
                    chunk: chunk_data.clone(),
                });

                current_chunk_start_bin = bin + 1;
                chunk_data.clear();

                // Микропауза 5мс, чтобы дать UI Dioxus плавно отрисовать кадр
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    });
}
