use std::path::Path;
use std::time::Duration;

use rikka_audio_decode::Sound;
use rikka_video::{
    EncodeSpeed, EncoderBackend, Error, Keyframes, Muxer, PacketConfig, Picture, PixelLayout,
    VideoPackets, first_frame, probe, trim,
};

use crate::support::{CHANNELS, FPS, HEIGHT, SAMPLE_RATE, WIDTH, recording, shade};

fn brightness(path: &Path) -> u8 {
    let picture = first_frame(path).expect("the first frame of the cut");
    let middle = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    picture.rgba8()[middle]
}

#[test]
fn trim_keeps_only_range() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Screencast.mp4");
    if !recording(&path, false) {
        return;
    }
    let cut = directory.path().join("Screencast cut.mp4");

    let keyframes = Keyframes::scan(&path).expect("the places it can be cut");
    let start = keyframes
        .nearest(Duration::from_secs(2))
        .expect("a keyframe to start from");
    let end = start + Duration::from_secs(2);
    let trimmed = trim(&path, &cut, start..end).expect("the cut");

    assert_eq!(trimmed.start, start, "the cut starts where it was asked to");
    assert!(
        trimmed.end <= end,
        "the cut keeps nothing the recording was to lose: {trimmed:?}"
    );
    assert!(
        trimmed.end + Duration::from_millis(500) > end,
        "the cut ends where it was asked to: {trimmed:?}"
    );

    let started = (start.as_secs_f64() * f64::from(FPS)).round() as u32;
    let shown = brightness(&cut);
    assert!(
        shown.abs_diff(shade(started)) < 12,
        "the cut opens on the frame it was cut at: {shown} against {}",
        shade(started)
    );

    let inside = Keyframes::scan(&cut).expect("the places the cut can be cut");
    assert_eq!(
        inside.times().first(),
        Some(&Duration::ZERO),
        "what is kept begins at the start of its own timeline"
    );
    let whole = std::fs::metadata(&path).expect("the recording").len();
    let part = std::fs::metadata(&cut).expect("the cut").len();
    assert!(part < whole, "the cut is smaller than the recording");
}

#[test]
fn trim_keeps_audio() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Screencast.mp4");
    if !recording(&path, true) {
        return;
    }
    let cut = directory.path().join("Screencast cut.mp4");

    let trimmed =
        trim(&path, &cut, Duration::from_secs(2)..Duration::from_secs(4)).expect("the cut");

    let sound = Sound::open(&cut).expect("the sound of the cut");
    println!("{:?} of sound in a {:?} cut", sound.duration(), trimmed);

    assert_eq!(sound.channels(), CHANNELS);
    assert_eq!(sound.sample_rate(), SAMPLE_RATE);
    assert!(
        sound.duration() > Duration::from_millis(1_500)
            && sound.duration() < Duration::from_millis(2_500),
        "the sound is as long as the cut: {:?}",
        sound.duration()
    );
    assert!(
        sound.frames() > 0,
        "the sound of the cut holds the samples it was recorded with"
    );
}

#[test]
fn empty_trim_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Screencast.mp4");
    if !recording(&path, false) {
        return;
    }
    let cut = directory.path().join("Screencast cut.mp4");

    let cases = [
        (
            Duration::from_secs(4)..Duration::from_secs(2),
            "a backwards cut",
        ),
        (
            Duration::from_secs(600)..Duration::from_secs(700),
            "a cut beyond the end",
        ),
    ];
    for (span, case) in cases {
        assert!(
            matches!(trim(&path, &cut, span), Err(Error::EmptyCut)),
            "{case} keeps nothing"
        );
    }
    assert!(
        matches!(
            trim(&path, &path, Duration::ZERO..Duration::from_secs(2)),
            Err(Error::OverSource)
        ),
        "a cut is never written over the recording it reads"
    );
}

#[test]
fn trim_ends_on_static_picture() {
    let config = PacketConfig::new(WIDTH, HEIGHT)
        .frame_rate(FPS)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Fastest)
        .keyframe_every(Duration::from_secs(1));
    let Ok(mut video) = VideoPackets::open(None, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
    else {
        return;
    };
    let pixels = [90u8, 90, 90, 255].repeat((WIDTH * HEIGHT) as usize);
    let mut coded = Vec::new();
    for at in [0.0, 0.5, 1.0, 9.0] {
        let picture = Picture {
            bytes: &pixels,
            width: WIDTH,
            height: HEIGHT,
            stride: (WIDTH * 4) as usize,
            layout: PixelLayout::Rgba,
        };
        coded.extend(
            video
                .encode(picture, Duration::from_secs_f64(at))
                .expect("a picture"),
        );
    }
    let stream = video.stream().expect("stream parameters");
    coded.extend(video.finish().expect("flush"));

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Still.mp4");
    let mut muxer = Muxer::create(&path, &[&stream]).expect("muxer");
    for packet in &coded {
        muxer.write(0, packet.sample()).expect("a picture");
    }
    muxer.finish().expect("trailer");

    let cut = directory.path().join("Still cut.mp4");
    let end = Duration::from_secs(3);
    let trimmed = trim(&path, &cut, Duration::ZERO..end).expect("the cut");
    assert_eq!(
        trimmed.end, end,
        "the still picture is kept only to the cut's end"
    );
    let length = probe(&cut)
        .expect("a readable cut")
        .duration
        .expect("a known length");
    assert!(
        length.abs_diff(end) < Duration::from_millis(200),
        "the cut lasts {length:?}, not {end:?}"
    );
}
