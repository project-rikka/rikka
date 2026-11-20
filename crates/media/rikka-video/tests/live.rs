use std::time::Duration;

use rikka_video::{
    EncodeSpeed, EncoderBackend, LiveCodec, LiveDecoder, PacketConfig, ParameterSets, Picture,
    PixelLayout, VideoPackets,
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 180;
const BITRATE: u64 = 800_000;

fn screen(index: u32, width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|pixel| {
            let x = pixel % width;
            let shade = ((x + index * 7) % 256) as u8;
            [shade, 255 - shade, 128, 255]
        })
        .collect()
}

fn units(frame: &[u8]) -> Vec<u8> {
    frame
        .windows(4)
        .filter(|window| window[..3] == [0, 0, 1])
        .map(|window| window[3] & 0x1F)
        .collect()
}

fn streaming() -> PacketConfig {
    PacketConfig::new(WIDTH, HEIGHT)
        .frame_rate(30)
        .bitrate(BITRATE)
        .backend(EncoderBackend::Software)
        .speed(EncodeSpeed::Fast)
        .parameter_sets(ParameterSets::InBand)
        .keyframes_on_request()
}

fn encoder(config: PacketConfig) -> Option<VideoPackets> {
    VideoPackets::open(None, config)
        .inspect_err(|error| eprintln!("skipping: nothing can encode here ({error})"))
        .ok()
}

fn picture(pixels: &[u8], (width, height): (u32, u32), layout: PixelLayout) -> Picture<'_> {
    Picture {
        bytes: pixels,
        width,
        height,
        stride: width as usize * 4,
        layout,
    }
}

#[test]
fn live_stream_starts_with_parameter_sets() {
    let Some(mut encoder) = encoder(streaming()) else {
        return;
    };
    let Ok(mut decoder) = LiveDecoder::open(LiveCodec::H264) else {
        return;
    };
    let source = (WIDTH * 2, HEIGHT * 2);
    let mut decoded = 0;
    let mut first = None;
    for index in 0..12 {
        let pixels = screen(index, source.0, source.1);
        let at = Duration::from_millis(u64::from(index) * 33);
        let coded = encoder
            .encode(picture(&pixels, source, PixelLayout::Bgra), at)
            .unwrap();
        for frame in coded {
            first.get_or_insert_with(|| frame.clone());
            for shown in decoder.decode(&frame.data, Some((160, 160))).unwrap() {
                assert_eq!((shown.width, shown.height), (160, 90));
                assert_eq!(shown.rgba.len(), 160 * 90 * 4);
                decoded += 1;
            }
        }
    }
    let first = first.expect("the encoder produced frames");
    assert!(first.keyframe);
    let kinds = units(&first.data);
    assert!(kinds.contains(&7) && kinds.contains(&8) && kinds.contains(&5));
    assert!(decoded >= 10, "{decoded} pictures came back");
}

#[test]
fn on_request_stream_keys_only_when_asked() {
    let Some(mut encoder) = encoder(streaming()) else {
        return;
    };
    let mut keyframes = Vec::new();
    for index in 0..40 {
        if index == 10 {
            encoder.set_bitrate(BITRATE / 2);
        }
        if index == 20 {
            encoder.request_keyframe();
        }
        let pixels = screen(index, WIDTH, HEIGHT);
        let at = Duration::from_millis(u64::from(index) * 33);
        let out = encoder
            .encode(picture(&pixels, (WIDTH, HEIGHT), PixelLayout::Rgba), at)
            .unwrap();
        assert_eq!(out.len(), 1, "frame {index}");
        if out[0].keyframe {
            keyframes.push(index);
        }
    }
    assert_eq!(keyframes, [0, 20], "with {}", encoder.name());
}
