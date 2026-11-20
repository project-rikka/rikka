use std::io::{Cursor, Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::support::generate_wav;
use crate::tones;
use rikka_audio_decode::{AudioFormat, Sound, StreamReader};

const HEAD: u64 = 256 * 1024;

struct MemoryStream {
    bytes: Cursor<Vec<u8>>,
    arrived: u64,
    starved: Arc<AtomicBool>,
}

impl MemoryStream {
    fn whole(bytes: Vec<u8>) -> MemoryStream {
        let arrived = bytes.len() as u64;
        MemoryStream {
            bytes: Cursor::new(bytes),
            arrived,
            starved: Arc::default(),
        }
    }

    fn arriving(bytes: Vec<u8>, arrived: u64) -> MemoryStream {
        MemoryStream {
            bytes: Cursor::new(bytes),
            arrived,
            starved: Arc::default(),
        }
    }
}

impl Read for MemoryStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let left = self.arrived.saturating_sub(self.bytes.position());
        if left == 0 && self.bytes.position() < self.bytes.get_ref().len() as u64 {
            self.starved.store(true, Ordering::Relaxed);
            return Err(std::io::Error::other("these bytes have not arrived"));
        }
        let len = buf.len().min(usize::try_from(left).unwrap_or(usize::MAX));
        self.bytes.read(&mut buf[..len])
    }
}

impl Seek for MemoryStream {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

impl StreamReader for MemoryStream {
    fn complete_len(&self) -> Option<u64> {
        let length = self.bytes.get_ref().len() as u64;
        (self.arrived == length).then_some(length)
    }
}

#[test]
fn decodes_wav_from_bytes() {
    let sound = rikka_audio_decode::read(generate_wav(44100, 4410))
        .format(AudioFormat::Wav)
        .decode()
        .unwrap();
    assert_eq!(sound.sample_rate(), 44100);
    assert_eq!(sound.channels(), 2);
    assert_eq!(sound.frames(), 4410);
    let duration = sound.duration();
    assert!((duration.as_secs_f64() - 0.1).abs() < 1e-6);
}

#[test]
fn auto_detects_wav_without_hint() {
    let sound = Sound::from_bytes(&generate_wav(48000, 480)).unwrap();
    assert_eq!(sound.sample_rate(), 48000);
    assert_eq!(sound.frames(), 480);
}

#[test]
fn audio_probe_skips_decoding() {
    let info = rikka_audio_decode::read(generate_wav(44100, 22050))
        .probe()
        .unwrap();

    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.channels, 2);
    let duration = info.duration.expect("wav reports a duration");
    assert!((duration.as_secs_f64() - 0.5).abs() < 1e-3, "{duration:?}");
}

#[test]
fn reader_probe_reports_bit_depth() {
    let info = *rikka_audio_decode::reader_decoder(MemoryStream::whole(generate_wav(96000, 9600)))
        .unwrap()
        .info();

    assert_eq!(info.sample_rate, 96000);
    assert_eq!(info.channels, 2);
    assert_eq!(info.bits_per_sample, Some(16));
}

#[test]
fn reader_probe_uses_ffmpeg() {
    let info = *rikka_audio_decode::reader_decoder(MemoryStream::whole(tones::WAVPACK.to_vec()))
        .unwrap()
        .info();

    assert_eq!(info.encoding.codec, rikka_audio_decode::Codec::WavPack);
    assert_eq!(info.frames, Some(tones::FRAMES));
    assert_eq!(info.bits_per_sample, Some(16));
}

#[test]
fn incomplete_reader_opens_from_what_arrived() {
    let stream = MemoryStream::arriving(generate_wav(48000, 48000 * 60), HEAD);
    let starved = Arc::clone(&stream.starved);
    let info = *rikka_audio_decode::reader_decoder(stream).unwrap().info();

    assert_eq!(info.sample_rate, 48000);
    assert!(!starved.load(Ordering::Relaxed));
}

#[test]
fn resampled_sound_changes_rate_not_duration() {
    let sound = rikka_audio_decode::read(generate_wav(44100, 44100))
        .format(AudioFormat::Wav)
        .decode()
        .unwrap();
    let resampled = sound.resampled(48000, rikka_resample::ResampleQuality::High);
    assert_eq!(resampled.sample_rate(), 48000);
    let delta = resampled.duration().as_secs_f64() - sound.duration().as_secs_f64();
    assert!(delta.abs() < 0.001, "duration drifted by {delta}");
}
#[test]
fn decoder_matches_full_decode_after_seek() {
    let wav = generate_wav(44100, 44100);
    let whole = rikka_audio_decode::read(wav.clone()).decode().unwrap();
    let mut decoder = rikka_audio_decode::read(wav).decoder().unwrap();
    assert_eq!((decoder.sample_rate(), decoder.channels()), (44100, 2));

    let reached = decoder.seek(std::time::Duration::from_millis(500)).unwrap();
    assert!((reached.as_secs_f64() - 0.5).abs() < 1e-6, "{reached:?}");
    let mut streamed = Vec::new();
    while streamed.len() < 4410 * 2 && decoder.read(&mut streamed).unwrap() {}

    let from = 22050 * 2;
    assert_eq!(streamed[..4410 * 2], whole.samples()[from..from + 4410 * 2]);
}
