use std::path::Path;
use std::process::Command;

use rikka_video::{EncoderBackend, EncoderConfig, VideoEncoder};

const THUMBNAILER: &str = env!("CARGO_BIN_EXE_rikka-video-thumbnailer");
const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;

fn clip(path: &Path) -> bool {
    let config = EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(10)
        .backend(EncoderBackend::Software);
    let Ok(mut encoder) = VideoEncoder::new(None, path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return false;
    };
    let frame: Vec<u8> = (0..WIDTH * HEIGHT)
        .flat_map(|index| {
            let shade = u8::try_from(index % WIDTH * 255 / WIDTH).unwrap_or(u8::MAX);
            [shade, 255 - shade, 128, 255]
        })
        .collect();
    for _ in 0..30 {
        encoder.push_frame(&frame).expect("push frame");
    }
    encoder.finish().expect("finish");
    true
}

fn thumbnailed(size: &str, input: &Path, output: &Path) -> bool {
    Command::new(THUMBNAILER)
        .args(["-s", size])
        .arg(input)
        .arg(output)
        .status()
        .expect("run thumbnailer")
        .success()
}

#[test]
fn video_thumbnail_fits_requested_size() {
    let directory = tempfile::tempdir().expect("create directory");
    let video = directory.path().join("clip.mp4");
    if !clip(&video) {
        return;
    }
    let output = directory.path().join("clip.png");

    assert!(thumbnailed("128", &video, &output));

    let made = rikka_image::read(&output).decode().expect("decode png");
    assert_eq!(made.dimensions(), (128, 72));
}

#[test]
fn non_video_fails_without_output() {
    let directory = tempfile::tempdir().expect("create directory");
    let fake = directory.path().join("fake.mp4");
    std::fs::write(&fake, b"not a video at all").expect("write file");
    let output = directory.path().join("fake.png");

    assert!(!thumbnailed("128", &fake, &output));
    assert!(!output.exists());
}
