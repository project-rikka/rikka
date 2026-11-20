use std::path::{Path, PathBuf};
use std::time::Duration;

use rikka_video::{
    EncodeSpeed, EncoderBackend, EncoderConfig, Keyframes, PixelSize, Still, Stills, VideoEncoder,
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;
const FPS: u32 = 10;
const FRAMES: u32 = 60;

fn level(index: u32) -> u8 {
    (16 + index * 3) as u8
}

fn index_of(still: &Still) -> u32 {
    let image = still.image.rgba8();
    let middle =
        ((still.image.height() / 2 * still.image.width() + still.image.width() / 2) * 4) as usize;
    ((f64::from(image[middle]) - 16.0) / 3.0).round() as u32
}

fn at(index: u32) -> Duration {
    Duration::from_millis(u64::from(index) * 1000 / u64::from(FPS))
}

fn greys() -> Option<(tempfile::TempDir, PathBuf)> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("greys.mp4");
    let config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(FPS)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Balanced)
        .quality(100);
    let Ok(mut encoder) = VideoEncoder::new(None, &path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return None;
    };
    for index in 0..FRAMES {
        let value = level(index);
        encoder
            .push_frame(&[value, value, value, 255].repeat((WIDTH * HEIGHT) as usize))
            .expect("push frame");
    }
    encoder.finish().expect("finish");
    Some((directory, path))
}

fn opened(path: &Path) -> Stills {
    Stills::open(path).expect("stills open")
}

#[test]
fn zero_reach_returns_covering_frame() {
    let Some((_directory, path)) = greys() else {
        return;
    };
    let mut stills = opened(&path);

    for seconds in [0.0, 0.35, 0.36, 0.4, 2.55, 5.95, 0.05, 3.0, 2.99, 4.2, 9.0] {
        let time = Duration::from_secs_f64(seconds);
        let index = ((seconds * f64::from(FPS) + 1e-6).floor() as u32).min(FRAMES - 1);
        let still = stills.still(time, Duration::ZERO).expect("a still");
        assert_eq!(still.pts, at(index), "the still shown at {time:?}");
        assert_eq!(index_of(&still), index, "the picture shown at {time:?}");
    }
}

#[test]
fn keyframe_within_reach_is_preferred() {
    let Some((_directory, path)) = greys() else {
        return;
    };
    let keyframes = Keyframes::scan(&path).expect("keyframes");
    let Some(&keyframe) = keyframes.times().iter().find(|time| !time.is_zero()) else {
        return;
    };
    let mut stills = opened(&path);
    let requested_at = keyframe.saturating_sub(at(3));

    let still = stills.still(requested_at, at(5)).expect("a still");

    assert_eq!(still.pts, keyframe);
    assert_eq!(index_of(&still), (keyframe.as_millis() / 100) as u32);
    let exact = stills.still(requested_at, Duration::ZERO).expect("a still");
    assert_eq!(exact.pts, requested_at);
}

#[test]
fn stills_fit_bounds_keeping_aspect() {
    let Some((_directory, path)) = greys() else {
        return;
    };
    let mut stills = opened(&path).with_max_size(PixelSize::new(32, 32));

    let still = stills.still(at(12), Duration::ZERO).expect("a still");

    assert_eq!((still.image.width(), still.image.height()), (32, 24));
    stills.set_max_size(PixelSize::new(16, 100));
    let still = stills.still(at(12), Duration::ZERO).expect("a still");
    assert_eq!((still.image.width(), still.image.height()), (16, 12));
}
