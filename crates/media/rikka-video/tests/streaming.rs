use std::time::Duration;

use rikka_gpu::Pixels;
use rikka_renderer::{DmabufSpec, Node, Renderer, Scene};
use rikka_types::{Color, Rect};
use rikka_video::{
    Coded, Feed, LiveCodec, LiveDecoder, PacketConfig, ParameterSets, VideoCodec, VideoPackets,
};

use super::{ARGB8888, exported, paint, spec_of, video_renderer};

const SOURCE: (u32, u32) = (1920, 1080);
const CANVAS: (u32, u32) = (1280, 720);
const FRAME: Duration = Duration::from_micros(16_667);
const BITRATE: u64 = 6_000_000;
const FRAMES: u32 = 24;
const REQUESTED: u32 = 8;
const RETUNED: u32 = 16;
const SQUARE: f32 = 240.0;
const STRIPED: (u32, u32) = (960, 540);
const SHRUNK: (u32, u32) = (320, 180);
const MOST_STRIPE_CONTRAST: f64 = 128.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Parameters,
    Idr,
    Filler,
    Other,
}

const STREAMED: [VideoCodec; 2] = [VideoCodec::H264, VideoCodec::Hevc];

fn units(data: &[u8], codec: VideoCodec) -> Vec<Unit> {
    data.windows(4)
        .filter(|window| window[..3] == [0, 0, 1])
        .map(|window| match codec {
            VideoCodec::H264 => match window[3] & 0x1f {
                7 | 8 => Unit::Parameters,
                5 => Unit::Idr,
                12 => Unit::Filler,
                _ => Unit::Other,
            },
            VideoCodec::Hevc | VideoCodec::Av1 | VideoCodec::Vp9 => match (window[3] >> 1) & 0x3f {
                32..=34 => Unit::Parameters,
                19 | 20 => Unit::Idr,
                38 => Unit::Filler,
                _ => Unit::Other,
            },
        })
        .collect()
}

fn opens_sequence(units: &[Unit], codec: VideoCodec) -> bool {
    let sets = match codec {
        VideoCodec::H264 => 2,
        VideoCodec::Hevc | VideoCodec::Av1 | VideoCodec::Vp9 => 3,
    };
    units.contains(&Unit::Idr)
        && units
            .iter()
            .filter(|unit| **unit == Unit::Parameters)
            .count()
            >= sets
}

fn interactive(codec: VideoCodec, (width, height): (u32, u32)) -> PacketConfig {
    PacketConfig::new(width, height)
        .frame_rate(60)
        .bitrate(BITRATE)
        .codec(codec)
        .feed(Feed::Dmabuf)
        .parameter_sets(ParameterSets::InBand)
        .keyframes_on_request()
}

fn opened(renderer: &Renderer, config: PacketConfig) -> Option<VideoPackets> {
    VideoPackets::open(Some(renderer.gpu()), config)
        .inspect_err(|error| eprintln!("{config:?} is not streamed from dmabufs here: {error}"))
        .ok()
}

fn live_codec(codec: VideoCodec) -> LiveCodec {
    match codec {
        VideoCodec::H264 => LiveCodec::H264,
        VideoCodec::Hevc => LiveCodec::Hevc,
        VideoCodec::Av1 => LiveCodec::Av1,
        VideoCodec::Vp9 => LiveCodec::Vp9,
    }
}

fn checked(frame: &Coded, index: u32, codec: VideoCodec) {
    let kinds = units(&frame.data, codec);
    assert!(
        !kinds.contains(&Unit::Filler),
        "{codec:?} frame {index} carries filler"
    );
    let keyframe = index == 0 || index == REQUESTED;
    assert_eq!(frame.keyframe, keyframe, "{codec:?} frame {index}");
    if keyframe {
        assert!(
            opens_sequence(&kinds, codec),
            "{codec:?} keyframe {index} is {kinds:?}"
        );
    }
    assert_eq!(
        frame.pts_us,
        (FRAME * index).as_micros() as i64,
        "{codec:?} frame {index}"
    );
}

#[test]
fn dmabuf_stream_keys_only_on_request() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let Some(mut exported) = exported(&mut renderer, SOURCE) else {
        return;
    };
    let mut scene = Scene::new();
    scene.set_background(Color::hex(0x203040));
    let square =
        scene.insert(Node::rect(Rect::new(0.0, 0.0, SQUARE, SQUARE)).fill(Color::hex(0xe0c040)));
    let mut streamed = Vec::new();
    for codec in STREAMED {
        let Some(mut encoder) = opened(&renderer, interactive(codec, CANVAS)) else {
            continue;
        };
        let (planes, modifier) = spec_of(&exported);
        let spec = DmabufSpec {
            width: SOURCE.0,
            height: SOURCE.1,
            fourcc: ARGB8888,
            modifier,
            planes: &planes,
        };
        let imported = encoder
            .import(&spec)
            .expect("the dmabuf imports for compute");
        drop(planes);
        let mut decoder = LiveDecoder::open(live_codec(codec)).expect("a live decoder");
        let mut decoded = 0;
        for index in 0..FRAMES {
            imported.wait_idle().expect("the last read finished");
            let step = index as f32 * 40.0;
            scene
                .edit(square)
                .rect(Rect::new(step, step / 2.0, SQUARE, SQUARE));
            paint(&mut renderer, &mut scene, &mut exported);
            if index == REQUESTED {
                encoder.request_keyframe();
            }
            if index == RETUNED {
                encoder.set_bitrate(BITRATE / 2);
            }
            let out = encoder
                .encode_imported(&imported, FRAME * index)
                .expect("an access unit");
            assert_eq!(
                out.len(),
                1,
                "{codec:?} frame {index} came out as it went in"
            );
            checked(&out[0], index, codec);
            for shown in decoder
                .decode(&out[0].data, None)
                .expect("the access unit decodes")
            {
                assert_eq!((shown.width, shown.height), CANVAS);
                decoded += 1;
            }
        }
        assert_eq!(decoded, FRAMES, "{codec:?}");
        streamed.push(codec);
    }
    if streamed.is_empty() {
        eprintln!("no codec is streamed from dmabufs on this GPU; nothing was checked");
    }
}

fn stripes((width, height): (u32, u32)) -> Vec<u8> {
    (0..width * height)
        .flat_map(|pixel| {
            let level = if pixel % width % 2 == 0 { 0 } else { 255 };
            [level, level, level, 255]
        })
        .collect()
}

fn contrast(picture: &[u8], (width, height): (u32, u32)) -> f64 {
    let rows = height / 4..height * 3 / 4;
    let columns = width / 4..width * 3 / 4;
    let count = rows.len() * (columns.len() - 1);
    let sum: f64 = rows
        .flat_map(|y| {
            columns.clone().skip(1).map(move |x| {
                let at = |x: u32| f64::from(picture[((y * width + x) * 4 + 1) as usize]);
                (at(x) - at(x - 1)).abs()
            })
        })
        .sum();
    sum / count as f64
}

#[test]
fn downscaled_dmabuf_averages_fine_detail() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let Some(mut exported) = exported(&mut renderer, STRIPED) else {
        return;
    };
    let texture = renderer
        .textures()
        .create(Pixels::rgba8(STRIPED.0, STRIPED.1, stripes(STRIPED)))
        .expect("the stripes upload");
    let mut scene = Scene::new();
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, STRIPED.0 as f32, STRIPED.1 as f32),
        texture,
    ));
    paint(&mut renderer, &mut scene, &mut exported);
    let Some((codec, mut encoder)) = STREAMED.into_iter().find_map(|codec| {
        opened(&renderer, interactive(codec, SHRUNK).bitrate(BITRATE * 2))
            .map(|encoder| (codec, encoder))
    }) else {
        return;
    };
    let (planes, modifier) = spec_of(&exported);
    let spec = DmabufSpec {
        width: STRIPED.0,
        height: STRIPED.1,
        fourcc: ARGB8888,
        modifier,
        planes: &planes,
    };
    let imported = encoder
        .import(&spec)
        .expect("the dmabuf imports for compute");
    let out = encoder
        .encode_imported(&imported, Duration::ZERO)
        .expect("an access unit");
    let mut decoder = LiveDecoder::open(live_codec(codec)).expect("a live decoder");
    let shown = decoder
        .decode(&out[0].data, None)
        .expect("the access unit decodes")
        .pop()
        .expect("a picture");
    assert_eq!((shown.width, shown.height), SHRUNK);
    let contrast = contrast(&shown.rgba, SHRUNK);
    assert!(
        contrast < MOST_STRIPE_CONTRAST,
        "one-pixel stripes shrunk threefold keep {contrast:.0} of 255 contrast"
    );
}
