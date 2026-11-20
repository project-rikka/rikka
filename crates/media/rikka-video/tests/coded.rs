use std::time::Duration;

use rikka_video::{
    AudioConfig, Coded, EncodeSpeed, EncoderBackend, Muxer, PacketConfig, Picture, PixelLayout,
    SoundPackets, VideoPackets, first_frame, probe,
};

use crate::support::{CHANNELS, FPS, FRAMES, HEIGHT, SAMPLE_RATE, WIDTH, shade, tone};

const KEYFRAMES_EVERY: Duration = Duration::from_secs(1);

fn flat(level: u8) -> Vec<u8> {
    [level, level, level, 255].repeat((WIDTH * HEIGHT) as usize)
}

fn coding() -> Option<(VideoPackets, Vec<Coded>)> {
    let config = PacketConfig::new(WIDTH, HEIGHT)
        .frame_rate(FPS)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Fastest)
        .keyframe_every(KEYFRAMES_EVERY);
    let mut video = VideoPackets::open(None, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
        .ok()?;
    let mut coded = Vec::new();
    for index in 0..FRAMES {
        let pixels = flat(shade(index));
        let picture = Picture {
            bytes: &pixels,
            width: WIDTH,
            height: HEIGHT,
            stride: (WIDTH * 4) as usize,
            layout: PixelLayout::Rgba,
        };
        let at = Duration::from_secs_f64(f64::from(index) / f64::from(FPS));
        let out = video.encode(picture, at).expect("a frame");
        assert_eq!(out.len(), 1, "picture {index} came out as it went in");
        coded.extend(out);
    }
    Some((video, coded))
}

#[test]
fn keyframes_follow_requested_interval() {
    let Some((video, mut coded)) = coding() else {
        return;
    };
    coded.extend(video.finish().expect("flush"));
    assert_eq!(coded.len(), FRAMES as usize);
    let keys: Vec<i64> = coded
        .iter()
        .filter(|packet| packet.keyframe)
        .map(|packet| packet.pts_us)
        .collect();
    assert_eq!(keys.first(), Some(&0));
    let every = KEYFRAMES_EVERY.as_micros() as i64;
    for pair in keys.windows(2) {
        assert!(
            pair[1] - pair[0] <= every,
            "keyframes {pair:?} lie too far apart"
        );
    }
    assert!(coded.windows(2).all(|pair| pair[0].dts_us < pair[1].dts_us));
}

#[test]
fn keyframe_packets_mux_into_playable_clip() {
    let Some((video, mut coded)) = coding() else {
        return;
    };
    let video_stream = video.stream().expect("video parameters");
    coded.extend(video.finish().expect("flush"));
    let mut sound = SoundPackets::open(AudioConfig::new(SAMPLE_RATE, CHANNELS)).expect("aac");
    let sound_stream = sound.stream().expect("sound parameters");
    let total = SAMPLE_RATE * FRAMES / FPS;
    let mut packets = sound.push(&tone(0, total)).expect("sound");
    packets.extend(sound.finish().expect("flush sound"));

    let start = coded
        .iter()
        .find(|packet| packet.keyframe && packet.pts_us >= 2_000_000)
        .expect("a keyframe past two seconds")
        .pts_us;
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("Replay.mp4");
    let mut muxer = Muxer::create(&path, &[&video_stream, &sound_stream]).expect("muxer");
    for packet in coded.iter().filter(|packet| packet.dts_us >= start) {
        muxer
            .write(0, packet.sample().shifted(start))
            .expect("a picture");
    }
    for packet in packets.iter().filter(|packet| packet.pts_us >= start) {
        muxer
            .write(1, packet.sample().shifted(start))
            .expect("sound");
    }
    muxer.finish().expect("trailer");

    let info = probe(&path).expect("a readable clip");
    assert_eq!((info.width, info.height), (WIDTH, HEIGHT));
    let length = info.duration.expect("a known length").as_secs_f64();
    let expected = f64::from(FRAMES) / f64::from(FPS) - start as f64 / 1e6;
    assert!(
        (length - expected).abs() < 0.3,
        "{length} s, not {expected} s"
    );

    let opening = first_frame(&path).expect("the clip opens on a picture");
    let index = (start * i64::from(FPS) / 1_000_000) as u32;
    let red = i32::from(opening.rgba8()[0]);
    assert!(
        (red - i32::from(shade(index))).abs() <= 6,
        "the clip opens on shade {red}, not {}",
        shade(index)
    );
}
