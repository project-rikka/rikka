#![allow(dead_code, reason = "each topic uses part of the shared setup")]

use rikka_sync::{Mutex, MutexGuard};
use std::path::Path;
use std::time::Duration;

use cut_project::{
    Appearance, ClipId, Edit, Media, Mix, Picture, Project, Rate, Redraw, Sound, Treatment,
};
use rikka_audio_decode::{WavDepth, WavWriter};
use rikka_image_edit::Imaging;
use rikka_video::{AudioConfig, EncodeSpeed, EncoderBackend, EncoderConfig, VideoEncoder};

pub(crate) const SAMPLE_RATE: u32 = 48_000;

static TURN: Mutex<()> = Mutex::new(());

pub(crate) struct Engine {
    pub(crate) imaging: Imaging,
    _turn: MutexGuard<'static, ()>,
}

pub(crate) fn engine() -> Engine {
    let turn = TURN.lock();
    Engine {
        imaging: Imaging::headless().expect("a Vulkan 1.3 device that can compute is required"),
        _turn: turn,
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        rikka_video::wait_for_closed_players();
    }
}

pub(crate) fn sound(path: &Path, samples: &[f32], channels: u16) -> Media {
    let mut writer =
        WavWriter::create(path, SAMPLE_RATE, channels, WavDepth::Pcm24).expect("a WAV file");
    writer.write(samples).expect("written samples");
    writer.finish().expect("a finished WAV file");
    let frames = samples.len() / usize::from(channels);
    Media::new(
        path,
        Duration::from_secs_f64(frames as f64 / f64::from(SAMPLE_RATE)),
    )
    .with_sound(Sound {
        sample_rate: SAMPLE_RATE,
        channels,
    })
}

pub(crate) fn click(frames: usize, at: usize) -> Vec<f32> {
    let mut samples = vec![0.0; frames];
    samples[at] = 1.0;
    samples
}

pub(crate) fn tone(frames: usize, level: f32) -> Vec<f32> {
    (0..frames)
        .map(|index| {
            level * (std::f32::consts::TAU * 441.0 * index as f32 / SAMPLE_RATE as f32).sin()
        })
        .collect()
}

pub(crate) fn film(path: &Path, size: (u32, u32), fps: u32, frames: u32) -> Option<Media> {
    let (width, height) = size;
    let config = EncoderConfig::new(width, height)
        .frame_rate(fps)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Fastest)
        .quality(100)
        .audio(AudioConfig::new(SAMPLE_RATE, 2));
    let Ok(mut encoder) = VideoEncoder::new(None, path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return None;
    };
    let per_frame = (SAMPLE_RATE / fps) as usize;
    let sound = tone(per_frame * frames as usize, 0.5);
    for index in 0..frames as usize {
        let stereo: Vec<f32> = sound[index * per_frame..(index + 1) * per_frame]
            .iter()
            .flat_map(|sample| [*sample, *sample])
            .collect();
        encoder.push_audio(&stereo).expect("pushed sound");
        encoder
            .push_frame(&[200, 40, 40, 255].repeat((width * height) as usize))
            .expect("pushed frame");
    }
    encoder.finish().expect("a finished film");
    Some(
        Media::new(
            path,
            Duration::from_secs_f64(f64::from(frames) / f64::from(fps)),
        )
        .with_picture(Picture {
            width,
            height,
            rate: Rate::new(fps, 1).expect("a rate"),
        })
        .with_sound(Sound {
            sample_rate: SAMPLE_RATE,
            channels: 2,
        }),
    )
}

pub(crate) fn placed(project: &Project, media: Media, at: i64, layer: usize) -> (Project, ClipId) {
    let (project, applied) = project
        .apply(&Edit::Place {
            media: vec![media],
            at,
            layer,
        })
        .expect("placing");
    (project, applied.selected.expect("the placed clip")[0])
}

pub(crate) fn treated(
    project: &Project,
    clip: ClipId,
    change: impl FnOnce(&mut Treatment),
) -> Project {
    let mut treatment = *project
        .locate(clip)
        .and_then(|(_, clip)| clip.treatment())
        .expect("footage");
    change(&mut treatment);
    project
        .apply(&Edit::Redraw {
            changes: vec![(clip, Appearance::Footage(treatment))],
            by: Redraw::Style,
            at: 0,
        })
        .expect("restyled")
        .0
}

pub(crate) fn mixed(volume: f32, pan: f32, muted: bool) -> Mix {
    Mix {
        volume,
        pan,
        muted,
        ..Mix::default()
    }
}
