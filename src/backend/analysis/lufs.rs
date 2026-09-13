use crate::shared::event_types::{PlayerEvent, StatusSender};
use ebur128::{EbuR128, Mode};
use rodio::Source;
use rodio::decoder::DecoderBuilder;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

/// Starts a background LUFS analysis task.
///
/// The analyzer sends:
/// - LUFS-M: momentary loudness, approximately 400 ms;
/// - LUFS-S: short-term loudness, approximately 3 seconds;
/// - LUFS-I: integrated programme loudness.
pub fn stream_loudness<T>(path: PathBuf, status_tx: T)
where
    T: Fn(PlayerEvent) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
{
    std::thread::spawn(move || {
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return,
        };

        let byte_len = match fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(_) => return,
        };

        let decoder = match DecoderBuilder::new()
            .with_data(BufReader::new(file))
            .with_byte_len(byte_len)
            .with_seekable(true)
            .with_coarse_seek(true)
            .build()
        {
            Ok(decoder) => decoder,
            Err(_) => return,
        };

        let channels = decoder.channels().get() as usize;
        let sample_rate = decoder.sample_rate().get();

        if channels == 0 || sample_rate == 0 {
            return;
        }

        let mut meter = match EbuR128::new(channels as u32, sample_rate, Mode::all()) {
            Ok(meter) => meter,
            Err(_) => return,
        };

        // One update for every 100 ms of decoded audio.
        let frames_per_update = (sample_rate as usize / 10).max(1);
        let samples_per_update = frames_per_update * channels;

        let mut buffer = Vec::with_capacity(samples_per_update);

        for sample in decoder {
            buffer.push(sample);

            if buffer.len() < samples_per_update {
                continue;
            }

            if meter.add_frames_f32(&buffer).is_err() {
                return;
            }

            buffer.clear();
        }

        let complete_samples = buffer.len() - buffer.len() % channels;

        if complete_samples > 0 && meter.add_frames_f32(&buffer[..complete_samples]).is_err() {
            return;
        }

        let _ = send_integrated_loudness(&meter, &status_tx);
    });
}

fn send_integrated_loudness<T>(meter: &EbuR128, status_tx: &T) -> bool
where
    T: Fn(PlayerEvent) -> Result<(), Box<dyn std::error::Error>>,
{
    let Some(integrated_lufs) = meter
        .loudness_global()
        .ok()
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
    else {
        return true;
    };

    let event = PlayerEvent::LoudnessMeasured {
        momentary_lufs: None,
        shortterm_lufs: None,
        integrated_lufs: Some(integrated_lufs),
    };

    eprintln!("LUFS-I: {integrated_lufs:.1}");

    status_tx(event).is_ok()
}

/* fn send_loudness_update<T>(meter: &EbuR128, status_tx: &T) -> bool
where
    T: Fn(PlayerEvent) -> Result<(), Box<dyn std::error::Error>>,
{
    let momentary_lufs = meter.loudness_momentary().ok().map(|value| value as f32);
    let shortterm_lufs = meter.loudness_shortterm().ok().map(|value| value as f32);
    let integrated_lufs = meter.loudness_global().ok().map(|value| value as f32);

    let event = PlayerEvent::LoudnessMeasured {
        momentary_lufs,
        shortterm_lufs,
        integrated_lufs,
    };

    eprintln!(
        "LUFS: momentary={:?}, shortterm={:?}, integrated={:?}",
        momentary_lufs, shortterm_lufs, integrated_lufs,
    );

    status_tx(event).is_ok()
} */

pub fn start_loudness_analysis(path: PathBuf, status_tx: &StatusSender) {
    let status_tx = status_tx.clone();

    stream_loudness(path, move |event| {
        status_tx
            .try_broadcast(event)
            .map(|_| ())
            .map_err(|error| error.into())
    });
}
