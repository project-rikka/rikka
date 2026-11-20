use std::path::{Path, PathBuf};

use rikka_video::{EncodeSpeed, EncoderBackend, EncoderConfig, VideoEncoder};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;
const FPS: u32 = 10;
const FRAMES: u32 = 60;
const DARK_FRAMES: u32 = 20;

fn black() -> Vec<u8> {
    [0, 0, 0, 255].repeat((WIDTH * HEIGHT) as usize)
}

fn stripes() -> Vec<u8> {
    (0..WIDTH * HEIGHT)
        .flat_map(|index| match (index % WIDTH) / 8 % 2 {
            0 => [230, 60, 30, 255],
            _ => [20, 90, 220, 255],
        })
        .collect()
}

fn clip(path: &Path, frame: impl Fn(u32) -> Vec<u8>) -> bool {
    let config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(FPS)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Balanced)
        .quality(100);
    let Ok(mut encoder) = VideoEncoder::new(None, path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return false;
    };
    for index in 0..FRAMES {
        encoder.push_frame(&frame(index)).expect("push frame");
    }
    encoder.finish().expect("finish");
    true
}

fn recorded(name: &str, frame: impl Fn(u32) -> Vec<u8>) -> Option<(tempfile::TempDir, PathBuf)> {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join(name);
    clip(&path, frame).then_some((directory, path))
}

fn spread(rgba: &[u8]) -> u8 {
    let reds = rgba.as_chunks::<4>().0.iter().map(|pixel| pixel[0]);
    let (low, high) = reds.fold((u8::MAX, u8::MIN), |(low, high), red| {
        (low.min(red), high.max(red))
    });
    high.saturating_sub(low)
}

#[test]
fn poster_skips_dark_opening() {
    let Some((_directory, path)) = recorded("opening.mp4", |index| match index < DARK_FRAMES {
        true => black(),
        false => stripes(),
    }) else {
        return;
    };

    let first = rikka_video::first_frame(&path).expect("the first frame");
    assert!(spread(&first.rgba8()) < 16, "the clip opens dark");

    let poster = rikka_video::poster(&path).decode().expect("a poster");
    assert_eq!(poster.dimensions(), (WIDTH, HEIGHT));
    assert!(
        spread(&poster.rgba8()) > 100,
        "the poster shows the stripes rather than the dark opening"
    );
}

#[test]
fn non_video_has_no_poster() {
    let directory = tempfile::tempdir().expect("a directory");
    let path = directory.path().join("notes.mp4");
    std::fs::write(&path, b"certainly not a video").expect("a file");

    assert!(rikka_video::poster(&path).decode().is_err());
    assert!(
        rikka_video::poster(directory.path().join("absent.mp4"))
            .decode()
            .is_err()
    );
}
