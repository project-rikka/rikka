use std::path::Path;

use rikka_video::{
    AudioConfig, EncodeSpeed, EncoderBackend, EncoderConfig, Error, VideoEncoder, first_frame,
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

fn config() -> EncoderConfig {
    EncoderConfig::new(WIDTH, HEIGHT)
        .frame_rate(10)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Fastest)
}

fn grey() -> Vec<u8> {
    [128, 128, 128, 255].repeat((WIDTH * HEIGHT) as usize)
}

fn encoder(path: &Path, config: EncoderConfig) -> Option<VideoEncoder> {
    VideoEncoder::new(None, path, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
        .ok()
}

#[test]
fn unwritable_destination_leaves_no_file() {
    let folder = tempfile::tempdir().unwrap();
    let probe = folder.path().join("probe.mp4");
    if encoder(&probe, config()).is_none() {
        return;
    }

    let missing = folder.path().join("missing").join("Screencast.mp4");
    let refused = VideoEncoder::new(None, &missing, config().audio(AudioConfig::new(48_000, 2)));
    assert!(
        matches!(refused, Err(Error::Av { .. })),
        "{:?}",
        refused.err()
    );
    assert!(!missing.exists());
}

#[test]
fn wrong_size_frame_rejected_recording_finishes() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("short.mp4");
    let Some(mut encoder) = encoder(&path, config()) else {
        return;
    };

    encoder.push_frame(&grey()).expect("a whole frame");
    assert!(encoder.push_frame(&grey()[4..]).is_err());
    encoder
        .push_frame(&grey())
        .expect("a whole frame after a refused one");
    let encoded = encoder.finish().expect("finish");
    assert_eq!(encoded.frames, 2);
    assert_eq!(
        first_frame(&path).expect("a playable file").dimensions(),
        (WIDTH, HEIGHT)
    );
}

#[test]
fn fast_start_puts_index_first() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("streamable.mp4");
    let Some(mut encoder) = encoder(&path, config().fast_start()) else {
        return;
    };
    for _ in 0..3 {
        encoder.push_frame(&grey()).expect("a frame");
    }
    encoder.finish().expect("finish");

    let bytes = std::fs::read(&path).expect("the movie was written");
    let at = |tag: &[u8]| bytes.windows(4).position(|window| window == tag);
    let (contents, pictures) = (at(b"moov"), at(b"mdat"));
    assert!(contents.is_some() && pictures.is_some());
    assert!(
        contents < pictures,
        "moov {contents:?} after mdat {pictures:?}"
    );
}
