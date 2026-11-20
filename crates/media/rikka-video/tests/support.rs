use std::path::Path;

use rikka_video::{AudioConfig, EncodeSpeed, EncoderBackend, EncoderConfig, VideoEncoder};

pub(crate) const WIDTH: u32 = 160;
pub(crate) const HEIGHT: u32 = 120;

pub(crate) const FPS: u32 = 10;
pub(crate) const FRAMES: u32 = 60;

const SHADE: u32 = 3;
const DARKEST: u32 = 40;

pub(crate) const SAMPLE_RATE: u32 = 48_000;
pub(crate) const CHANNELS: u16 = 2;
const TONE: f32 = 440.0;
const LOUDNESS: f32 = 0.5;

pub(crate) fn shade(index: u32) -> u8 {
    (DARKEST + index * SHADE) as u8
}

fn frame(index: u32) -> Vec<u8> {
    let level = shade(index);
    [level, level, level, 255]
        .repeat((WIDTH * HEIGHT) as usize)
        .to_vec()
}

pub(crate) fn tone(first: u64, frames: u32) -> Vec<f32> {
    (0..frames)
        .flat_map(|offset| {
            let at = (first + u64::from(offset)) as f32 / SAMPLE_RATE as f32;
            let sample = (at * TONE * std::f32::consts::TAU).sin() * LOUDNESS;
            [sample; CHANNELS as usize]
        })
        .collect()
}

pub(crate) fn recording(path: &Path, sound: bool) -> bool {
    let mut config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(FPS)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Fastest);
    if sound {
        config = config.audio(AudioConfig::new(SAMPLE_RATE, CHANNELS));
    }
    let Ok(mut encoder) = VideoEncoder::new(None, path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return false;
    };
    let per_frame = SAMPLE_RATE / FPS;
    for index in 0..FRAMES {
        encoder.push_frame(&frame(index)).expect("push frame");
        if sound {
            encoder
                .push_audio(&tone(u64::from(index * per_frame), per_frame))
                .expect("push audio");
        }
    }
    encoder.finish().expect("finish");
    true
}
