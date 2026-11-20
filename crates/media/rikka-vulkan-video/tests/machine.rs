use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rikka_gpu::DmabufPlane;
use rikka_renderer::{
    DamageSource, DmabufFence, DmabufSpec, ExportedDmabuf, Node, Renderer, Scene,
};
use rikka_types::{Color, Rect};
use rikka_video::{LiveCodec, LiveDecoder};
use rikka_vulkan_video::{Codec, Encoder, Rate, Request};

const RENDER_NODE: &str = "/dev/dri/renderD128";
const WIDTH: u32 = 642;
const HEIGHT: u32 = 362;
const ARGB8888: u32 = u32::from_le_bytes(*b"AR24");
const ROUNDING: i32 = 10;
const COLOURS: [u32; 6] = [0x3366cc, 0x3366cc, 0x20a040, 0xe0d010, 0xe0d010, 0x802080];

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
        .app_name("rikka-vulkan-video-machine")
        .drm_node(major as u32, minor as u32)
        .compute_support()
        .video_encode()
        .build()
        .and_then(|device| Renderer::new(&device))
        .inspect_err(|error| eprintln!("no renderer encodes video here ({error}); skipping"))
        .ok()
}

fn exported(renderer: &mut Renderer) -> ExportedDmabuf {
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
    renderer
        .export_dmabuf(WIDTH, HEIGHT, ARGB8888, &modifiers)
        .expect("an exported buffer")
}

fn paint(renderer: &mut Renderer, exported: &mut ExportedDmabuf, colour: u32) {
    let mut scene = Scene::new();
    scene.set_background(Color::hex(colour));
    scene.insert(
        Node::rect(Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32)).fill(Color::hex(colour)),
    );
    scene.insert(Node::rect(Rect::new(40.0, 40.0, 120.0, 60.0)).fill(Color::hex(0xffffff)));
    let mut output = renderer.create_dmabuf_output(DamageSource::CompleteForBuffer);
    let drawn = renderer
        .draw(&mut scene, output.with_target(&mut exported.target))
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

fn centre(picture: &rikka_video::LivePicture) -> [i32; 3] {
    let at = ((picture.height / 2 * picture.width + picture.width * 3 / 4) * 4) as usize;
    [
        i32::from(picture.rgba[at]),
        i32::from(picture.rgba[at + 1]),
        i32::from(picture.rgba[at + 2]),
    ]
}

fn assert_colour(picture: &rikka_video::LivePicture, expected: u32, what: &str) {
    let expected = [
        (expected >> 16) as i32 & 0xff,
        (expected >> 8) as i32 & 0xff,
        expected as i32 & 0xff,
    ];
    let got = centre(picture);
    for channel in 0..3 {
        assert!(
            (got[channel] - expected[channel]).abs() <= ROUNDING,
            "{what}: came back {got:?}, not {expected:?}"
        );
    }
}

fn live(codec: Codec) -> LiveCodec {
    match codec {
        Codec::H264 => LiveCodec::H264,
        Codec::H265 => LiveCodec::Hevc,
    }
}

#[test]
fn recovery_frames_predict_from_the_chosen_reference() {
    let Some(mut renderer) = video_renderer() else {
        return;
    };
    let mut source_buffer = exported(&mut renderer);
    let mut checked = false;
    for codec in [Codec::H265, Codec::H264] {
        if !Encoder::supports(renderer.gpu(), codec) {
            eprintln!("{codec} is not encoded here");
            continue;
        }
        let mut encoder = Encoder::builder(renderer.gpu(), codec, WIDTH, HEIGHT)
            .frame_rate(60)
            .references(8)
            .rate(Rate {
                bitrate: 8_000_000,
                ..Rate::default()
            })
            .build()
            .expect("the encoder opens");
        let planes: Vec<DmabufPlane<'_>> = source_buffer
            .planes
            .iter()
            .map(|plane| DmabufPlane {
                offset: plane.offset,
                stride: plane.stride,
                fd: plane.fd.as_fd(),
            })
            .collect();
        let spec = DmabufSpec {
            width: WIDTH,
            height: HEIGHT,
            fourcc: ARGB8888,
            modifier: source_buffer.format.modifier,
            planes: &planes,
        };
        let source = encoder.import(&spec).expect("the capture imports");
        drop(planes);
        let mut decoder = LiveDecoder::open(live(codec)).expect("the oracle decoder opens");
        let mut previous = None;
        for (index, colour) in COLOURS.into_iter().enumerate() {
            source.wait_idle().expect("the last conversion finished");
            paint(&mut renderer, &mut source_buffer, colour);
            let request = match (index, previous) {
                (4, _) => Request::predicted(2),
                (5, Some(previous)) => Request::predicted(previous).constant(20),
                (_, Some(previous)) => Request::predicted(previous),
                (_, None) => Request::intra(),
            };
            if index == 2 {
                encoder.set_rate(Rate {
                    bitrate: 4_000_000,
                    ..Rate::default()
                });
            }
            let encoded = encoder
                .encode(&source, request)
                .expect("the picture encodes");
            assert_eq!(encoded.frame, index as u32);
            assert_eq!(encoded.intra, index == 0, "{codec} frame {index}");
            assert!(
                encoded.data.starts_with(&[0, 0, 0, 1]) || encoded.data.starts_with(&[0, 0, 1]),
                "{codec} frame {index} is an Annex B access unit"
            );
            if index == 4 {
                assert_eq!(encoded.reference, 2);
                assert!(encoder.holds(2));
            }
            previous = Some(encoded.frame);
            if index == 3 {
                continue;
            }
            let pictures = decoder
                .decode(&encoded.data, None)
                .unwrap_or_else(|error| panic!("{codec} frame {index} decodes: {error}"));
            assert_eq!(pictures.len(), 1, "{codec} frame {index} shows at once");
            assert_eq!(
                (pictures[0].width, pictures[0].height),
                (WIDTH & !1, HEIGHT & !1)
            );
            assert_colour(&pictures[0], colour, &format!("{codec} frame {index}"));
        }
        checked = true;
    }
    if !checked {
        eprintln!("no codec is encoded with Vulkan Video here; nothing was checked");
    }
}
