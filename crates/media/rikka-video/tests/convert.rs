use std::path::Path;
use std::time::Duration;

use rikka_audio_decode::Sound;
use rikka_types::Flow;
use rikka_video::{
    AudioCodec, AudioConfig, AudioEncoder, AudioTarget, Error, Handling, PixelSize, VideoCodec,
    VideoTarget, convert, first_frame, sample,
};

use crate::support::{CHANNELS, HEIGHT, SAMPLE_RATE, WIDTH, recording, tone};

const RECORDED: Duration = Duration::from_secs(6);

fn lasts_about(path: &Path, expected: Duration) {
    let sound = Sound::open(path).expect("the sound decodes");
    let off = sound.duration().abs_diff(expected);
    assert!(
        off < Duration::from_millis(150),
        "{} lasts {:?}, not {expected:?}",
        path.display(),
        sound.duration()
    );
    assert_eq!(sound.channels(), CHANNELS);
}

#[test]
fn matroska_copies_every_stream() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("clip.mp4");
    if !recording(&source, true) {
        return;
    }
    let remuxed = folder.path().join("clip.mkv");

    let converted = convert(&source, &remuxed)
        .video(VideoTarget::new(VideoCodec::H264))
        .audio(AudioTarget::new(AudioCodec::Aac))
        .run()
        .expect("the remux");

    assert_eq!(converted.video, Some(Handling::Copied));
    assert_eq!(converted.audio, Some(Handling::Copied));
    assert_eq!(
        first_frame(&remuxed).expect("a picture").dimensions(),
        (WIDTH, HEIGHT)
    );
    lasts_about(&remuxed, RECORDED);
}

#[test]
fn webm_encodes_within_bounds() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("clip.mp4");
    if !recording(&source, true) {
        return;
    }
    let encoded = folder.path().join("clip.webm");

    let converted = convert(&source, &encoded)
        .video(VideoTarget::new(VideoCodec::Vp9).with_max_size(PixelSize::new(80, 80)))
        .audio(AudioTarget::new(AudioCodec::Opus))
        .run()
        .expect("the encode");

    assert_eq!(converted.video, Some(Handling::Encoded));
    assert_eq!(converted.audio, Some(Handling::Encoded));
    assert_eq!(
        first_frame(&encoded).expect("a picture").dimensions(),
        (80, 60)
    );
    lasts_about(&encoded, RECORDED);
}

#[test]
fn fitting_audio_is_stream_copied() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("clip.mp4");
    if !recording(&source, true) {
        return;
    }

    for (name, codec, handled) in [
        ("sound.m4a", AudioCodec::Aac, Handling::Copied),
        ("sound.flac", AudioCodec::Flac, Handling::Encoded),
        ("sound.mp3", AudioCodec::Mp3, Handling::Encoded),
    ] {
        let taken = folder.path().join(name);
        let converted = convert(&source, &taken)
            .audio(AudioTarget::new(codec))
            .run()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(converted.video, None, "{name}");
        assert_eq!(converted.audio, Some(handled), "{name}");
        lasts_about(&taken, RECORDED);
    }
}

#[test]
fn conversion_cancels_and_never_overwrites_source() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("clip.mp4");
    if !recording(&source, true) {
        return;
    }

    let stopped = convert(&source, folder.path().join("clip.webm"))
        .video(VideoTarget::new(VideoCodec::Vp9))
        .on_progress(|_| Flow::Cancel)
        .run();
    assert!(matches!(stopped, Err(Error::Cancelled)), "{stopped:?}");

    let over = convert(&source, &source)
        .video(VideoTarget::new(VideoCodec::H264))
        .run();
    assert!(matches!(over, Err(Error::OverSource)), "{over:?}");
}

#[test]
fn video_sampled_at_fixed_steps() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("clip.mp4");
    if !recording(&source, false) {
        return;
    }

    let mut taken = Vec::new();
    sample(&source)
        .with_max_size(PixelSize::new(80, 80))
        .every(Duration::from_millis(500))
        .each(|at, picture| {
            taken.push((at, picture.dimensions()));
            Flow::Continue
        })
        .expect("the samples");

    let times: Vec<Duration> = taken.iter().map(|(at, _)| *at).collect();
    let expected: Vec<Duration> = (0..12)
        .map(|step| Duration::from_millis(500 * step))
        .collect();
    assert_eq!(times, expected);
    assert!(taken.iter().all(|(_, size)| *size == (80, 60)));
}

#[test]
fn every_audio_codec_round_trips() {
    let folder = tempfile::tempdir().unwrap();
    let second = tone(0, SAMPLE_RATE);

    for (name, codec) in [
        ("tone.m4a", AudioCodec::Aac),
        ("tone.opus", AudioCodec::Opus),
        ("tone.mp3", AudioCodec::Mp3),
        ("tone.ogg", AudioCodec::Vorbis),
        ("tone.flac", AudioCodec::Flac),
        ("alac.m4a", AudioCodec::Alac),
    ] {
        let path = folder.path().join(name);
        let mut encoder = AudioEncoder::new(
            &path,
            AudioConfig::new(SAMPLE_RATE, CHANNELS)
                .codec(codec)
                .quality(60),
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        encoder.push(&second).expect("the samples");
        encoder.finish().expect("the file");
        lasts_about(&path, Duration::from_secs(1));
    }
}
