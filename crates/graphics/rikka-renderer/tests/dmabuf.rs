use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use gbm::{BufferObjectFlags, Format as GbmFormat, Modifier};
use rikka_gpu::{DRM_FORMAT_MOD_LINEAR, DmabufPlane};
use rikka_renderer::{Blur, DamageSource, DmabufFence, DmabufSpec, Node, NodeId, Renderer, Scene};
use rikka_types::{Color, Rect};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;
const XRGB8888: u32 = GbmFormat::Xrgb8888 as u32;
const FILL: u32 = 0x3366cc;

const RENDER_NODE: &str = "/dev/dri/renderD128";

fn render_node(app_name: &str) -> Option<(gbm::Device<File>, Renderer)> {
    if !Path::new(RENDER_NODE).exists() {
        eprintln!("{RENDER_NODE} is absent; skipping");
        return None;
    }
    let node = OpenOptions::new()
        .read(true)
        .write(true)
        .open(RENDER_NODE)
        .expect("the render node opens");
    let dev = node.metadata().expect("render node metadata").rdev();
    let (major, minor) = (libc::major(dev), libc::minor(dev));
    rikka_test_renderer::init_tracing();
    let renderer = Renderer::new(
        &rikka_gpu::Gpu::builder()
            .app_name(app_name)
            .drm_node(major, minor)
            .build()
            .expect("a Vulkan device drives the render node"),
    )
    .expect("a Vulkan device drives the render node");
    let nodes = renderer
        .gpu()
        .drm_nodes()
        .expect("the driver reports its DRM nodes");
    assert!(
        nodes.matches((major, minor)),
        "the selected GPU is not the one behind {RENDER_NODE}: {nodes:?}"
    );
    Some((
        gbm::Device::new(node).expect("a GBM device on the render node"),
        renderer,
    ))
}

fn drawn_and_sampled(renderer: &Renderer) -> Option<u64> {
    let sampleable: Vec<u64> = renderer
        .gpu()
        .dmabuf_texture_formats()
        .iter()
        .filter(|format| format.fourcc == XRGB8888)
        .map(|format| format.modifier)
        .collect();
    assert!(!sampleable.is_empty(), "XRGB8888 cannot be sampled at all");
    let modifier = renderer
        .gpu()
        .dmabuf_render_formats()
        .iter()
        .filter(|format| format.fourcc == XRGB8888)
        .map(|format| format.modifier)
        .find(|modifier| sampleable.contains(modifier));
    if modifier.is_none() {
        eprintln!("no XRGB8888 layout is both renderable and sampleable; skipping");
    }
    modifier
}

fn fill_bytes() -> [u8; 3] {
    [
        ((FILL >> 16) & 0xff) as u8,
        ((FILL >> 8) & 0xff) as u8,
        (FILL & 0xff) as u8,
    ]
}

fn sampled(renderer: &mut Renderer, buffer: &Swapped) -> Vec<u8> {
    let planes = buffer.planes();
    let texture = renderer
        .textures()
        .import_dmabuf(&buffer.spec(&planes))
        .expect("the dmabuf imports as a sampled texture");
    let mut target = renderer
        .create_target(WIDTH, HEIGHT)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::TRANSPARENT);
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        texture,
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("the imported texture draws");
    let pixels = renderer
        .read_pixels(&mut target)
        .expect("the target reads back");
    renderer.textures().destroy(texture);
    pixels
}

struct Swapped {
    bo: gbm::BufferObject<()>,
    fds: Vec<OwnedFd>,
    modifier: u64,
}

impl Swapped {
    fn new(gbm: &gbm::Device<File>, modifier: u64) -> Self {
        let bo = gbm
            .create_buffer_object_with_modifiers2::<()>(
                WIDTH,
                HEIGHT,
                GbmFormat::Xrgb8888,
                [Modifier::from(modifier)].into_iter(),
                BufferObjectFlags::RENDERING,
            )
            .expect("an XRGB8888 buffer object in a layout the GPU can render");
        assert_eq!(u64::from(bo.modifier()), modifier);
        let fds = (0..bo.plane_count() as i32)
            .map(|plane| bo.fd_for_plane(plane).expect("the buffer exports a dmabuf"))
            .collect();
        Self { bo, fds, modifier }
    }

    fn planes(&self) -> Vec<DmabufPlane<'_>> {
        self.fds
            .iter()
            .enumerate()
            .map(|(index, fd)| DmabufPlane {
                offset: self.bo.offset(index as i32),
                stride: self.bo.stride_for_plane(index as i32),
                fd: fd.as_fd(),
            })
            .collect()
    }

    fn spec<'a>(&self, planes: &'a [DmabufPlane<'a>]) -> DmabufSpec<'a> {
        DmabufSpec {
            width: WIDTH,
            height: HEIGHT,
            fourcc: XRGB8888,
            modifier: self.modifier,
            planes,
        }
    }
}

#[test]
fn renders_into_imported_dmabuf() {
    let _gpu_lock = rikka_test_renderer::lock_gpu();
    let Some((gbm, mut renderer)) = render_node("rikka-dmabuf-test") else {
        return;
    };

    let formats = renderer.gpu().dmabuf_render_formats();
    assert!(
        !formats.is_empty(),
        "the GPU reports no importable scanout layouts"
    );
    let xrgb: Vec<u64> = formats
        .iter()
        .filter(|format| format.fourcc == XRGB8888)
        .map(|format| format.modifier)
        .collect();
    assert!(!xrgb.is_empty(), "XRGB8888 is not importable at all");
    let modifier = xrgb
        .iter()
        .copied()
        .find(|modifier| *modifier == DRM_FORMAT_MOD_LINEAR)
        .unwrap_or(xrgb[0]);

    let buffer = Swapped::new(&gbm, modifier);
    let mut target = renderer
        .import_dmabuf(&buffer.spec(&buffer.planes()))
        .expect("the dmabuf imports as a render target");
    assert_eq!((target.width(), target.height()), (WIDTH, HEIGHT));

    let mut output = renderer.create_dmabuf_output(DamageSource::SinceLastFrame);
    let mut scene = Scene::new();
    scene.set_background(Color::hex(FILL));
    scene.insert(
        Node::rect(Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32)).fill(Color::hex(FILL)),
    );

    let frame = renderer
        .draw(&mut scene, output.with_target(&mut target))
        .expect("the scene renders into the dmabuf");
    assert!(
        frame.output.rendered(),
        "the first frame into a buffer always has damage"
    );

    drop(target);

    if modifier != DRM_FORMAT_MOD_LINEAR {
        eprintln!("layout {modifier:#018x} is not linear; skipping the pixel check");
        return;
    }
    let centre = buffer
        .bo
        .map(WIDTH / 2, HEIGHT / 2, 1, 1, |mapping| {
            let pixels = mapping.buffer();
            [pixels[0], pixels[1], pixels[2]]
        })
        .expect("a linear buffer object maps for reading");
    let mut expected = fill_bytes();
    expected.reverse();
    for (channel, (got, want)) in centre.iter().zip(expected).enumerate() {
        assert!(
            got.abs_diff(want) <= 1,
            "channel {channel} of the scanout buffer is {got}, expected {want}"
        );
    }
}

#[test]
fn samples_imported_dmabuf() {
    let _gpu_lock = rikka_test_renderer::lock_gpu();
    let Some((gbm, mut renderer)) = render_node("rikka-dmabuf-texture-test") else {
        return;
    };
    let Some(modifier) = drawn_and_sampled(&renderer) else {
        return;
    };

    let buffer = Swapped::new(&gbm, modifier);
    {
        let mut target = renderer
            .import_dmabuf(&buffer.spec(&buffer.planes()))
            .expect("the dmabuf imports as a render target");
        let mut output = renderer.create_dmabuf_output(DamageSource::SinceLastFrame);
        let mut scene = Scene::new();
        scene.set_background(Color::hex(FILL));
        renderer
            .draw(&mut scene, output.with_target(&mut target))
            .expect("the scene renders into the dmabuf");
    }

    let pixels = sampled(&mut renderer, &buffer);
    let centre = ((HEIGHT / 2) as usize * WIDTH as usize + (WIDTH / 2) as usize) * 4;
    let got = [pixels[centre], pixels[centre + 1], pixels[centre + 2]];
    for (channel, (got, want)) in got.iter().zip(fill_bytes()).enumerate() {
        assert!(
            got.abs_diff(want) <= 1,
            "channel {channel} of the sampled buffer is {got}, expected {want}"
        );
    }
    assert_eq!(pixels[centre + 3], 0xff, "an X format samples as opaque");
}

#[test]
fn idle_buffer_accumulates_missed_damage() {
    const MISSED: usize = 12;
    const SQUARE: f32 = 16.0;

    let _gpu_lock = rikka_test_renderer::lock_gpu();
    let Some((gbm, mut renderer)) = render_node("rikka-dmabuf-age-test") else {
        return;
    };
    let Some(modifier) = drawn_and_sampled(&renderer) else {
        return;
    };

    let swapped: Vec<Swapped> = (0..3).map(|_| Swapped::new(&gbm, modifier)).collect();
    let mut targets: Vec<_> = swapped
        .iter()
        .map(|buffer| {
            renderer
                .import_dmabuf(&buffer.spec(&buffer.planes()))
                .expect("the dmabuf imports as a render target")
        })
        .collect();

    let mut output = renderer.create_dmabuf_output(DamageSource::CompleteForBuffer);
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let squares: Vec<_> = (0..MISSED)
        .map(|at| {
            let x = 4.0 + at as f32 * (SQUARE + 4.0);
            scene.insert(Node::rect(Rect::new(x, 8.0, SQUARE, SQUARE)).fill(Color::BLACK))
        })
        .collect();
    for target in &mut targets {
        renderer
            .draw(&mut scene, output.with_target(target))
            .expect("the first frame renders");
    }
    for (at, square) in squares.iter().enumerate() {
        scene.edit(*square).fill(Color::hex(FILL));
        renderer
            .draw(&mut scene, output.with_target(&mut targets[at % 2]))
            .expect("a frame renders into one of the two busy buffers");
    }
    renderer
        .draw(&mut scene, output.with_target(&mut targets[2]))
        .expect("the idle buffer renders again");
    drop(targets);

    let pixels = sampled(&mut renderer, &swapped[2]);
    let expected = fill_bytes();
    for at in 0..MISSED {
        let x = 4 + at * (SQUARE as usize + 4) + SQUARE as usize / 2;
        let pixel = (16 * WIDTH as usize + x) * 4;
        let got = [pixels[pixel], pixels[pixel + 1], pixels[pixel + 2]];
        assert!(
            got.iter()
                .zip(expected)
                .all(|(got, want)| got.abs_diff(want) <= 1),
            "square {at} changed while the buffer sat idle but reads {got:?}, expected {expected:?}"
        );
    }
}

const ARGB8888: u32 = GbmFormat::Argb8888 as u32;

fn glass_scene() -> (Scene, NodeId) {
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.9, 0.85, 0.8));
    let mut stripe = None;
    for at in 0..8 {
        let x = at as f32 * 32.0;
        let shade = at as f32 / 8.0;
        let id = scene.insert(
            Node::rect(Rect::new(x, 0.0, 16.0, HEIGHT as f32)).fill(Color::rgb(
                shade,
                0.3,
                1.0 - shade,
            )),
        );
        stripe.get_or_insert(id);
    }
    scene.insert(
        Node::rounded_rect(Rect::new(80.0, 40.0, 96.0, 48.0), 12.0)
            .fill(Color::WHITE.with_alpha(0.2))
            .backdrop(Blur::with_radius(8.0)),
    );
    (scene, stripe.expect("the scene has stripes"))
}

fn wait_readable(fence: &OwnedFd) {
    let mut watched = libc::pollfd {
        fd: std::os::fd::AsRawFd::as_raw_fd(fence),
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&raw mut watched, 1, 5_000) };
    assert_eq!(ready, 1, "the frame finished within five seconds");
}

fn exact(renderer: &mut Renderer, recolour: Color) -> Vec<u8> {
    let (mut scene, stripe) = glass_scene();
    scene.edit(stripe).fill(recolour);
    let mut target = renderer
        .create_target(WIDTH, HEIGHT)
        .expect("an offscreen target");
    renderer
        .draw(&mut scene, &mut target)
        .expect("the scene draws offscreen");
    renderer
        .read_pixels(&mut target)
        .expect("the target reads back")
}

#[test]
fn cropped_export_matches_exact_target() {
    const BUCKET: (u32, u32) = (WIDTH + 128, HEIGHT + 128);
    let recolour = Color::rgb(0.1, 0.9, 0.2);

    let _gpu_lock = rikka_test_renderer::lock_gpu();
    let Some((_gbm, mut renderer)) = render_node("rikka-dmabuf-export-test") else {
        return;
    };
    let sampleable: Vec<u64> = renderer
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
        .filter(|format| format.fourcc == ARGB8888 && sampleable.contains(&format.modifier))
        .map(|format| format.modifier)
        .collect();
    if modifiers.is_empty() {
        eprintln!("no ARGB8888 layout is both exported and sampled; skipping");
        return;
    }

    let exported = renderer
        .export_dmabuf(BUCKET.0, BUCKET.1, ARGB8888, &modifiers)
        .expect("the GPU exports a buffer in an offered layout");
    assert!(modifiers.contains(&exported.format.modifier));
    assert_eq!(exported.format.fourcc, ARGB8888);
    let mut target = exported.target;
    let (mut scene, stripe) = glass_scene();
    let mut output = renderer.create_dmabuf_output(DamageSource::SinceLastFrame);
    let first = renderer
        .draw(
            &mut scene,
            output.with_target(&mut target).visible(WIDTH, HEIGHT),
        )
        .expect("the first frame draws into the exported buffer");
    assert!(first.output.rendered());
    scene.edit(stripe).fill(recolour);
    let mut draw = output.with_target(&mut target).visible(WIDTH, HEIGHT);
    if let Some(DmabufFence::SyncFile(fence)) = first.fence {
        draw = draw.with_release_fence(fence);
    }
    let second = renderer
        .draw(&mut scene, draw)
        .expect("the second frame waits for the first and draws");
    assert!(second.output.rendered());
    if let Some(fence) = second.fence {
        wait_readable(&fence.into_readable());
    }
    drop(target);

    let planes: Vec<DmabufPlane<'_>> = exported
        .planes
        .iter()
        .map(|plane| DmabufPlane {
            offset: plane.offset,
            stride: plane.stride,
            fd: plane.fd.as_fd(),
        })
        .collect();
    let texture = renderer
        .textures()
        .import_dmabuf(&DmabufSpec {
            width: BUCKET.0,
            height: BUCKET.1,
            fourcc: ARGB8888,
            modifier: exported.format.modifier,
            planes: &planes,
        })
        .expect("the exported buffer imports as a texture");
    let mut shown = Scene::new();
    shown.set_background(Color::TRANSPARENT);
    shown.insert(Node::image(
        Rect::new(0.0, 0.0, BUCKET.0 as f32, BUCKET.1 as f32),
        texture,
    ));
    let mut readable = renderer
        .create_target(BUCKET.0, BUCKET.1)
        .expect("an offscreen target");
    renderer
        .draw(&mut shown, &mut readable)
        .expect("the exported buffer draws as a texture");
    let drawn = renderer
        .read_pixels(&mut readable)
        .expect("the target reads back");
    let expected = exact(&mut renderer, recolour);

    for y in 0..HEIGHT as usize {
        for x in 0..WIDTH as usize {
            let got = &drawn[(y * BUCKET.0 as usize + x) * 4..][..4];
            let want = &expected[(y * WIDTH as usize + x) * 4..][..4];
            assert!(
                got.iter()
                    .zip(want)
                    .all(|(got, want)| got.abs_diff(*want) <= 3),
                "pixel {x},{y} of the cropped frame is {got:?}, the exact frame has {want:?}"
            );
        }
    }
}
