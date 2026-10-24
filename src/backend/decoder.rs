use super::filters::LiveFilteredSource;
use super::gain::LiveGainSource;
use super::meter::MeteredSource;
use super::spatial::LiveSpatialSource;
use super::state::AudioState;
use crate::shared::event_types::AudioError;
use rodio::Source;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use crate::backend::timestretch::TimeStretchSource;
use rodio::{Decoder, decoder::DecoderBuilder};

use std::fs;

// ---------------------------------------------------------------------------
// Decoding helpers
// ---------------------------------------------------------------------------

// V2
pub(crate) fn open_file_with_decoder(path: &Path) -> Result<Decoder<BufReader<File>>, AudioError> {
    let file = File::open(path)
        .map_err(|_| AudioError::FileNotFound(path.to_string_lossy().into_owned()))?;

    let len = fs::metadata(path)
        .map_err(|_| AudioError::FileCorrupted)?
        .len();

    let decoder = DecoderBuilder::new()
        .with_data(BufReader::new(file))
        .with_byte_len(len)
        .with_seekable(true)
        .with_coarse_seek(true)
        // .with_hint("mp3")
        .build()
        .map_err(|_| AudioError::FileCorrupted)?;

    Ok(decoder)
}

// V2
fn append_source<S>(state: &AudioState, source: S)
where
    S: Source + Send + 'static,
{
    state.meter.reset();

    let gained = LiveGainSource::new(source, state.playback.gain.clone());

    let filtered = LiveFilteredSource::new(
        gained,
        state.effects.low_pass.clone(),
        state.effects.high_pass.clone(),
    );

    let spatial = LiveSpatialSource::new(
        filtered,
        state.effects.mid_side.clone(),
        state.effects.stereo_width.clone(),
        state.effects.pan.clone(),
    );

    let metered = MeteredSource::new(spatial, state.meter.clone());

    state.player.append(metered);
}

// V2
pub(crate) fn build_audio_pipeline(
    state: &AudioState,
    mut decoder: Decoder<BufReader<File>>,
    position: Duration,
) -> Result<(), AudioError> {
    // seek before wsola
    decoder
        .try_seek(position)
        .map_err(|_| AudioError::SeekFailed)?;

    let source = TimeStretchSource::new(
        decoder,
        state.playback.speed_shared.clone(),
        state.playback.preserve_pitch_shared.clone(),
    )
    .map_err(|_| AudioError::RenderAudioFailed)?;

    append_source(state, source);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::open_file_with_decoder;
    use crate::backend::timestretch::TimeStretchSource;
    use hound::{SampleFormat, WavSpec, WavWriter};
    use rodio::Source;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32};
    use std::time::Duration;

    fn write_24_bit_wav() -> PathBuf {
        let path = std::env::temp_dir().join("wave_player_24bit_48k_test.wav");
        let spec = WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 24,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::create(&path, spec).unwrap();

        for frame in 0..144_000 {
            let value = (frame as f32 / 48_000.0 * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            let sample = (value * 8_388_607.0) as i32;
            writer.write_sample(sample).unwrap();
            writer.write_sample(sample).unwrap();
        }

        writer.finalize().unwrap();
        path
    }

    #[test]
    fn decoder_should_read_24_bit_48k_to_eof_and_seek_back() {
        let path = write_24_bit_wav();
        let mut decoder = open_file_with_decoder(&path).unwrap();

        assert_eq!(decoder.total_duration(), Some(Duration::from_secs(3)));
        assert_eq!(decoder.by_ref().count(), 144_000 * 2);

        decoder.try_seek(Duration::ZERO).unwrap();
        assert!(decoder.next().is_some());

        let speed = Arc::new(AtomicU32::new(1.0_f32.to_bits()));
        let preserve_pitch = Arc::new(AtomicBool::new(false));
        let decoder = open_file_with_decoder(&path).unwrap();
        let mut stretched = TimeStretchSource::new(decoder, speed, preserve_pitch).unwrap();
        let mut finished = false;

        for _ in 0..144_000 * 2 * 4 {
            if stretched.next().is_none() {
                finished = true;
                break;
            }
        }

        assert!(finished, "24-bit/48 kHz source never finished");

        let _ = fs::remove_file(path);
    }
}
