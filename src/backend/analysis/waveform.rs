use crate::shared::event_types::{PlayerEvent, WaveformEnvelope};
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

pub fn stream_waveform_detail<T>(
    path: PathBuf,
    channels: u16,
    sample_rate: u32,
    total_frames: usize,
    start_frame: usize,
    end_frame: usize,
    point_count: usize,
    request_id: u64,
    status_tx: T,
) where
    T: Fn(PlayerEvent) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
{
    if channels == 0
        || sample_rate == 0
        || total_frames == 0
        || start_frame >= end_frame
        || point_count == 0
    {
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

        let end_frame = end_frame.min(total_frames);
        let frame_count = end_frame.saturating_sub(start_frame);

        if frame_count == 0 {
            return;
        }

        let point_count = point_count.min(frame_count).max(1);
        let channels_count = channels as usize;

        let mut points = vec![
            WaveformEnvelope {
                min: 1.0,
                max: -1.0,
            };
            point_count
        ];

        for frame_index in 0..end_frame {
            let mut frame_min = 1.0_f32;
            let mut frame_max = -1.0_f32;
            let mut complete_frame = true;

            for _ in 0..channels_count {
                match decoder.next() {
                    Some(sample) => {
                        frame_min = frame_min.min(sample);
                        frame_max = frame_max.max(sample);
                    }
                    None => {
                        complete_frame = false;
                        break;
                    }
                }
            }

            if !complete_frame || frame_index < start_frame {
                continue;
            }

            let relative_frame = frame_index - start_frame;

            let point_index = relative_frame
                .saturating_mul(point_count)
                .checked_div(frame_count)
                .unwrap_or(0)
                .min(point_count - 1);

            let point = &mut points[point_index];

            point.min = point.min.min(frame_min);
            point.max = point.max.max(frame_max);
        }

        for point in &mut points {
            if point.min > point.max {
                point.min = 0.0;
                point.max = 0.0;
            }
        }

        let _ = status_tx(PlayerEvent::WaveformDetailReady {
            request_id,
            start_frame,
            end_frame,
            total_frames,
            sample_rate,
            points,
        });
    });
}
