use crate::shared::event_types::PlayerEvent;
use rodio::decoder::DecoderBuilder;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Duration;

/// Starts a background task that calculates waveform peaks and sends them
/// to the UI in ordered chunks.
pub fn stream_waveform<T>(
    path: PathBuf,
    channels: u16,
    sample_rate: u32,
    duration: Duration,
    num_bins: usize,
    status_tx: T,
) where
    T: Fn(PlayerEvent) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
{
    let channels_count = channels as usize;

    if channels_count == 0 || sample_rate == 0 || num_bins == 0 {
        return;
    }

    std::thread::spawn(move || {
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return,
        };

        let byte_len = match fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(_) => return,
        };

        let mut decoder = match DecoderBuilder::new()
            .with_data(BufReader::new(file))
            .with_byte_len(byte_len)
            .build()
        {
            Ok(decoder) => decoder,
            Err(_) => return,
        };

        let estimated_total_frames =
            duration.as_secs_f64().mul_add(sample_rate as f64, 0.0) as usize;

        if estimated_total_frames == 0 {
            return;
        }

        let bins_per_chunk = 10;
        let mut chunk = Vec::with_capacity(bins_per_chunk);
        let mut chunk_start_bin = 0;

        let mut decoder_finished = false;

        for bin in 0..num_bins {
            let start_frame = bin.saturating_mul(estimated_total_frames) / num_bins;

            let end_frame = (bin + 1).saturating_mul(estimated_total_frames) / num_bins;

            let frames_in_bin = end_frame.saturating_sub(start_frame);
            let mut bin_peak = 0.0_f32;

            for _ in 0..frames_in_bin {
                if decoder_finished {
                    break;
                }

                let mut frame_peak = 0.0_f32;
                let mut samples_read = 0;

                for _ in 0..channels_count {
                    match decoder.next() {
                        Some(sample) => {
                            frame_peak = frame_peak.max(sample.abs());
                            samples_read += 1;
                        }
                        None => {
                            decoder_finished = true;
                            break;
                        }
                    }
                }

                if samples_read > 0 {
                    bin_peak = bin_peak.max(frame_peak);
                }
            }

            // Keep waveform values inside the UI-supported range.
            chunk.push(bin_peak.clamp(0.0, 1.0));

            if chunk.len() == bins_per_chunk || bin == num_bins - 1 {
                let event_chunk = std::mem::take(&mut chunk);

                let result = status_tx(PlayerEvent::WaveformChunkReady {
                    start_bin: chunk_start_bin,
                    total_bins: num_bins,
                    chunk: event_chunk,
                });

                if result.is_err() {
                    return;
                }

                chunk = Vec::with_capacity(bins_per_chunk);
                chunk_start_bin = bin + 1;
            }
        }
    });
}
