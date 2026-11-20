use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::Duration;

use rikka_gpu::{DmabufPlane, Pixels};
use rikka_renderer::{
    DamageSource, DmabufFence, DmabufSpec, ExportedDmabuf, Node, Renderer, Scene,
};
use rikka_types::{Color, Flow, Rect};
use rikka_video::{
    EncoderConfig, Feed, Muxer, PacketConfig, Previewer, VideoCodec, VideoEncoder, VideoPackets,
    first_frame, sample,
};

mod streaming;

const RENDER_NODE: &str = "/dev/dri/renderD128";
const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const CODED_FROM_DMABUFS: [VideoCodec; 2] = [VideoCodec::H264, VideoCodec::Hevc];
const ARGB8888: u32 = u32::from_le_bytes(*b"AR24");
const FILL: u32 = 0x3366cc;
const RIGHT: u32 = 0xcc6633;
const FRAMES: u32 = 12;
const STEP: Duration = Duration::from_millis(100);
const ROUNDING: i32 = 6;
const QUALITY: u8 = 70;
const MARGIN: u32 = 64;
const MOVES: u32 = 20;
const MIN_SHARPNESS: f64 = 30.0;

fn video_renderer() -> Option<Renderer> {
    if !Path::new(RENDER_NODE).exists() {
        eprintln!("{RENDER_NODE} is absent; skipping");
        return None;
    }
    let device = std::fs::metadata(RENDER_NODE)
        .expect("render node metadata")
        .rdev();
    let major = ((device >> 32) & 0xffff_f000) | ((device >> 8) & 0xfff);
    let minor = ((device >> 12) & 0xffff_ff00) | (device & 0xff);
    rikka_gpu::Gpu::builder()
        .app_name("rikka-video-machine")
        .drm_node(major as u32, minor as u32)
        .compute_support()
        .video_encode()
        .build()
        .and_then(|device| Renderer::new(&device))
        .inspect_err(|error| eprintln!("no renderer codes video here ({error}); skipping"))
        .ok()
}

fn filled(renderer: &mut Renderer) -> Option<ExportedDmabuf> {
    let mut scene = Scene::new();
    scene.set_background(Color::hex(FILL));
    scene.insert(
        Node::rect(Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32)).fill(Color::hex(FILL)),
    );
    let mut exported = exported(renderer, (WIDTH, HEIGHT))?;
    paint(renderer, &mut scene, &mut exported);
    Some(exported)
}

fn detail_at(width: u32, height: u32, (dx, dy): (u32, u32)) -> Vec<u8> {
    (0..height)
        .flat_map(|y| {
            (0..width).flat_map(move |x| {
                let (x, y) = (x + dx, y + dy);
                let stroke = (x / 3 + y / 7) % 5 == 0 || (x * y) % 13 == 0 || (x + 2 * y) % 17 < 2;
                let level = match stroke {
                    true => 24,
                    false => 232,
                };
                [level, level, level, 255]
            })
        })
        .collect()
}

fn shift(index: u32) -> (u32, u32) {
    (index * 3, index * 2)
}

fn exported(renderer: &mut Renderer, (width, height): (u32, u32)) -> Option<ExportedDmabuf> {
    let sampled: Vec<u64> = renderer
        .gpu()
        .dmabuf_texture_formats()
        .iter()
        .filter(|format| format.fourcc == ARGB8888)
        .map(|format| format.modifier)
        .collect();
    let modifiers: Vec<u64> = renderer
        .gpu()
        .dmabuf_export_formats()
        .iter()
        .filter(|format| format.fourcc == ARGB8888 && sampled.contains(&format.modifier))
        .map(|format| format.modifier)
        .collect();
    if modifiers.is_empty() {
        eprintln!("no ARGB8888 layout is both exported and sampled; skipping");
        return None;
    }
    Some(
        renderer
            .export_dmabuf(width, height, ARGB8888, &modifiers)
            .expect("an exported buffer"),
    )
}

fn paint(renderer: &mut Renderer, scene: &mut Scene, exported: &mut ExportedDmabuf) {
    let mut output = renderer.create_dmabuf_output(DamageSource::CompleteForBuffer);
    let drawn = renderer
        .draw(scene, output.with_target(&mut exported.target))
        .expect("the scene draws");
    if let Some(DmabufFence::SyncFile(fence)) = drawn.fence {
        wait_readable(&fence);
    }
}

fn wait_readable(fence: &OwnedFd) {
    let mut watched = libc::pollfd {
        fd: fence.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&raw mut watched, 1, 5_000) };
    assert_eq!(ready, 1, "the fill finished within five seconds");
}

fn spec_of(exported: &ExportedDmabuf) -> (Vec<DmabufPlane<'_>>, u64) {
    let planes = exported
        .planes
        .iter()
        .map(|plane| DmabufPlane {
            offset: plane.offset,
            stride: plane.stride,
            fd: plane.fd.as_fd(),
        })
        .collect();
    (planes, exported.format.modifier)
}

fn coded_clip(
    renderer: &Renderer,
    spec: &DmabufSpec<'_>,
    codec: VideoCodec,
    canvas: (u32, u32),
    folder: &Path,
) -> Option<rikka_image::Image> {
    let config = PacketConfig::new(canvas.0, canvas.1)
        .frame_rate(10)
        .codec(codec)
        .quality(QUALITY)
        .feed(Feed::Dmabuf);
    let mut video = match VideoPackets::open(Some(renderer.gpu()), config) {
        Ok(video) => video,
        Err(error) => {
            eprintln!("{codec:?} is not coded from dmabufs here: {error}");
            return None;
        }
    };
    let imported = video.import(spec).expect("the dmabuf imports for compute");
    let mut coded = Vec::new();
    for index in 0..FRAMES {
        imported.wait_idle().expect("the last read finished");
        let out = video
            .encode_imported(&imported, STEP * index)
            .expect("a coded picture");
        assert_eq!(
            out.len(),
            1,
            "{codec:?} picture {index} came out as it went in"
        );
        coded.extend(out);
    }
    let stream = video.stream().expect("stream parameters");
    coded.extend(video.finish().expect("flush"));
    assert_eq!(coded.len(), FRAMES as usize, "{codec:?}");
    assert!(coded[0].keyframe, "{codec:?} opens on a keyframe");

    let path = folder.join(format!("{codec:?}-{}x{}.mp4", canvas.0, canvas.1));
    let mut muxer = Muxer::create(&path, &[&stream]).expect("muxer");
    for packet in &coded {
        muxer.write(0, packet.sample()).expect("write");
    }
    muxer.finish().expect("trailer");
    let picture = first_frame(&path).expect("the clip decodes");
    assert_eq!(picture.dimensions(), canvas);
    Some(picture)
}

fn assert_colour(picture: &rikka_image::Image, at: (u32, u32), expected: u32, what: &str) {
    let (width, _) = picture.dimensions();
    let pixels = picture.rgba8();
    let start = ((at.1 * width + at.0) * 4) as usize;
    let expected = [
        (expected >> 16) & 0xff,
        (expected >> 8) & 0xff,
        expected & 0xff,
    ];
    for (channel, expected) in expected.into_iter().enumerate() {
        let got = i32::from(pixels[start + channel]);
        assert!(
            (got - expected as i32).abs() <= ROUNDING,
            "{what}: channel {channel} at {at:?} came back {got}, not {expected}"
        );
    }
}

#[test]
fn dmabuf_encodes_on_gpu_with_correct_colour() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let Some(exported) = filled(&mut renderer) else {
        return;
    };
    let (planes, modifier) = spec_of(&exported);
    let spec = DmabufSpec {
        width: WIDTH,
        height: HEIGHT,
        fourcc: ARGB8888,
        modifier,
        planes: &planes,
    };
    let folder = tempfile::tempdir().unwrap();
    let mut coded_any = false;
    for codec in CODED_FROM_DMABUFS {
        let Some(picture) = coded_clip(&renderer, &spec, codec, (WIDTH, HEIGHT), folder.path())
        else {
            continue;
        };
        assert_colour(
            &picture,
            (WIDTH / 2, HEIGHT / 2),
            FILL,
            &format!("{codec:?}"),
        );
        coded_any = true;
    }
    if !coded_any {
        eprintln!("no codec is coded from dmabufs on this GPU; nothing was checked");
    }
}

#[test]
fn smaller_dmabuf_is_letterboxed() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let Some(exported) = filled(&mut renderer) else {
        return;
    };
    let (planes, modifier) = spec_of(&exported);
    let spec = DmabufSpec {
        width: WIDTH,
        height: HEIGHT,
        fourcc: ARGB8888,
        modifier,
        planes: &planes,
    };
    let folder = tempfile::tempdir().unwrap();
    let canvas = (WIDTH * 2, HEIGHT * 2);
    let Some(picture) = coded_clip(&renderer, &spec, VideoCodec::H264, canvas, folder.path())
    else {
        return;
    };
    assert_colour(&picture, (canvas.0 / 2, canvas.1 / 2), FILL, "centre");
    for corner in [(8, 8), (canvas.0 - 8, canvas.1 - 8)] {
        assert_colour(&picture, corner, 0x000000, "margin");
    }
}

fn psnr(picture: &rikka_image::Image, reference: &[u8]) -> f64 {
    let got = picture.rgba8();
    let squared: f64 = got
        .as_chunks::<4>()
        .0
        .iter()
        .zip(reference.as_chunks::<4>().0)
        .map(|(got, reference)| (f64::from(got[0]) - f64::from(reference[0])).powi(2))
        .sum();
    let mean = squared / f64::from(WIDTH * HEIGHT);
    10.0 * (255.0 * 255.0 / mean.max(1e-9)).log10()
}

fn last_picture(path: &Path) -> rikka_image::Image {
    let mut last = None;
    sample(path)
        .every(STEP)
        .each(|_, picture| {
            last = Some(picture);
            Flow::Continue
        })
        .expect("the clip decodes");
    last.expect("the clip holds pictures")
}

fn scrolled(renderer: &mut Renderer, codec: VideoCodec, folder: &Path) -> Option<f64> {
    let mut exported = exported(renderer, (WIDTH, HEIGHT))?;
    let wide = (WIDTH + MARGIN, HEIGHT + MARGIN);
    let texture = renderer
        .textures()
        .create(Pixels::rgba8(
            wide.0,
            wide.1,
            detail_at(wide.0, wide.1, (0, 0)),
        ))
        .expect("the detail uploads");
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let node = scene.insert(Node::image(
        Rect::new(0.0, 0.0, wide.0 as f32, wide.1 as f32),
        texture,
    ));
    let config = PacketConfig::new(WIDTH, HEIGHT)
        .frame_rate(10)
        .codec(codec)
        .quality(QUALITY)
        .feed(Feed::Dmabuf);
    let mut video = match VideoPackets::open(Some(renderer.gpu()), config) {
        Ok(video) => video,
        Err(error) => {
            eprintln!("{codec:?} is not coded from dmabufs here: {error}");
            return None;
        }
    };
    let (planes, modifier) = spec_of(&exported);
    let spec = DmabufSpec {
        width: WIDTH,
        height: HEIGHT,
        fourcc: ARGB8888,
        modifier,
        planes: &planes,
    };
    let imported = video.import(&spec).expect("the dmabuf imports for compute");
    drop(planes);
    let mut coded = Vec::new();
    for index in 0..MOVES {
        imported.wait_idle().expect("the last read finished");
        let (dx, dy) = shift(index);
        scene.edit(node).rect(Rect::new(
            -(dx as f32),
            -(dy as f32),
            wide.0 as f32,
            wide.1 as f32,
        ));
        paint(renderer, &mut scene, &mut exported);
        coded.extend(
            video
                .encode_imported(&imported, STEP * index)
                .expect("a coded picture"),
        );
    }
    let stream = video.stream().expect("stream parameters");
    coded.extend(video.finish().expect("flush"));
    let path = folder.join(format!("{codec:?}-scrolled.mp4"));
    let mut muxer = Muxer::create(&path, &[&stream]).expect("muxer");
    for packet in &coded {
        muxer.write(0, packet.sample()).expect("write");
    }
    muxer.finish().expect("trailer");
    let last = last_picture(&path);
    Some(psnr(&last, &detail_at(WIDTH, HEIGHT, shift(MOVES - 1))))
}

#[test]
fn scrolling_detail_stays_sharp() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let folder = tempfile::tempdir().unwrap();
    for codec in CODED_FROM_DMABUFS {
        let Some(sharpness) = scrolled(&mut renderer, codec, folder.path()) else {
            continue;
        };
        assert!(
            sharpness >= MIN_SHARPNESS,
            "{codec:?} keeps {sharpness:.1} dB of a scrolled picture"
        );
    }
}

#[test]
fn cropped_dmabuf_records_only_its_region() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let Some(mut exported) = exported(&mut renderer, (WIDTH, HEIGHT)) else {
        return;
    };
    let half = WIDTH / 2;
    let mut scene = Scene::new();
    scene.set_background(Color::hex(FILL));
    scene.insert(
        Node::rect(Rect::new(half as f32, 0.0, half as f32, HEIGHT as f32)).fill(Color::hex(RIGHT)),
    );
    paint(&mut renderer, &mut scene, &mut exported);
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("Region.mp4");
    let config = EncoderConfig::new(half, HEIGHT)
        .frame_rate(10)
        .quality(QUALITY)
        .feed(Feed::Dmabuf);
    let mut encoder = match VideoEncoder::new(Some(renderer.gpu()), &path, config) {
        Ok(encoder) => encoder,
        Err(error) => {
            eprintln!("nothing records dmabufs here: {error}");
            return;
        }
    };
    let (planes, modifier) = spec_of(&exported);
    let spec = DmabufSpec {
        width: WIDTH,
        height: HEIGHT,
        fourcc: ARGB8888,
        modifier,
        planes: &planes,
    };
    let imported = encoder
        .import(&spec)
        .expect("the dmabuf imports for compute")
        .cropped((half, 0), (half, HEIGHT));
    for index in 0..FRAMES {
        imported.wait_idle().expect("the last read finished");
        assert!(
            encoder
                .push_imported_at(&imported, STEP * index)
                .expect("a coded picture"),
            "picture {index} has a place of its own"
        );
    }
    let recorded = encoder.finish().expect("the recording closes");
    assert_eq!(recorded.frames, u64::from(FRAMES));
    let picture = first_frame(&path).expect("the recording decodes");
    assert_eq!(picture.dimensions(), (half, HEIGHT));
    for at in [
        (8, HEIGHT / 2),
        (half / 2, HEIGHT / 2),
        (half - 8, HEIGHT / 2),
    ] {
        assert_colour(&picture, at, RIGHT, "cropped region");
    }
}

#[test]
fn dmabuf_preview_keeps_colour_and_aspect() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let Some(exported) = filled(&mut renderer) else {
        return;
    };
    let previewer = Previewer::new(renderer.gpu()).expect("a previewer");
    let (planes, modifier) = spec_of(&exported);
    let spec = DmabufSpec {
        width: WIDTH,
        height: HEIGHT,
        fourcc: ARGB8888,
        modifier,
        planes: &planes,
    };
    let imported = previewer.import(&spec).expect("the dmabuf imports");
    let preview = previewer
        .preview(&imported, (WIDTH / 4, WIDTH / 4))
        .expect("a preview");
    assert_eq!((preview.width, preview.height), (WIDTH / 4, HEIGHT / 4));
    let picture =
        rikka_image::Image::from_rgba8(preview.width, preview.height, preview.rgba).unwrap();
    assert_colour(&picture, (WIDTH / 8, HEIGHT / 8), FILL, "preview");
}
