use std::fs::{File, OpenOptions};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use gbm::{BufferObject, BufferObjectFlags, Format as GbmFormat, Modifier};
use rikka_gpu::DmabufPlane;
use rikka_renderer::{BackdropCache, Blur, DamageSource, DmabufSpec, Node, Renderer, Scene};
use rikka_types::{Color, Rect};

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;
const XRGB8888: u32 = GbmFormat::Xrgb8888 as u32;
const RENDER_NODE: &str = "/dev/dri/renderD128";

const QUIET_FRAMES: u32 = 620;

const GROUND: Color = Color::rgb(0.8, 0.2, 0.07);
const PANE: Rect = Rect::new(64.0, 16.0, 128.0, 96.0);
const CARET: Rect = Rect::new(4.0, 4.0, 8.0, 12.0);

fn whole() -> Rect {
    Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32)
}

fn frame(scene: &mut Scene, damage: &[Rect], glass: bool) {
    scene.restart();
    for rect in damage {
        scene.insert(Node::rect(*rect).fill(GROUND).clip(*rect));
        if glass && let Some(clip) = PANE.intersection(rect) {
            scene.insert(
                Node::rounded_rect(PANE, 12.0)
                    .fill(Color::WHITE.with_alpha(0.1))
                    .clip(clip)
                    .backdrop(Blur::with_radius(24.0))
                    .backdrop_cache(BackdropCache::stale(1, [whole()])),
            );
        }
    }
}

struct Buffer {
    _bo: BufferObject<()>,
    fds: Vec<OwnedFd>,
    offsets: Vec<u32>,
    strides: Vec<u32>,
    width: u32,
    height: u32,
}

impl Buffer {
    fn make(gbm: &gbm::Device<File>, modifier: u64, width: u32, height: u32) -> Self {
        let bo = gbm
            .create_buffer_object_with_modifiers2::<()>(
                width,
                height,
                GbmFormat::Xrgb8888,
                [Modifier::from(modifier)].into_iter(),
                BufferObjectFlags::RENDERING,
            )
            .expect("an XRGB8888 buffer object the GPU can render into");
        let planes = 0..bo.plane_count() as i32;
        Self {
            fds: planes
                .clone()
                .map(|plane| bo.fd_for_plane(plane).expect("the buffer exports a dmabuf"))
                .collect(),
            offsets: planes.clone().map(|plane| bo.offset(plane)).collect(),
            strides: planes.map(|plane| bo.stride_for_plane(plane)).collect(),
            width,
            height,
            _bo: bo,
        }
    }

    fn planes(&self) -> Vec<DmabufPlane<'_>> {
        self.fds
            .iter()
            .enumerate()
            .map(|(index, fd)| DmabufPlane {
                offset: self.offsets[index],
                stride: self.strides[index],
                fd: fd.as_fd(),
            })
            .collect()
    }
}

fn spec<'a>(buffer: &'a Buffer, planes: &'a [DmabufPlane<'a>], modifier: u64) -> DmabufSpec<'a> {
    DmabufSpec {
        width: buffer.width,
        height: buffer.height,
        fourcc: XRGB8888,
        modifier,
        planes,
    }
}

fn open(
    name: &str,
) -> Option<(
    rikka_test_renderer::GpuLock,
    Renderer,
    gbm::Device<File>,
    u64,
)> {
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

    rikka_test_renderer::init_tracing();
    let gpu_lock = rikka_test_renderer::lock_gpu();
    let renderer = Renderer::new(
        &rikka_gpu::Gpu::builder()
            .app_name(name)
            .drm_node(libc::major(dev), libc::minor(dev))
            .build()
            .expect("a Vulkan device drives the render node"),
    )
    .expect("a Vulkan device drives the render node");

    let sampleable: Vec<u64> = renderer
        .gpu()
        .dmabuf_texture_formats()
        .iter()
        .filter(|format| format.fourcc == XRGB8888)
        .map(|format| format.modifier)
        .collect();
    let modifier = renderer
        .gpu()
        .dmabuf_render_formats()
        .iter()
        .filter(|format| format.fourcc == XRGB8888)
        .map(|format| format.modifier)
        .find(|modifier| sampleable.contains(modifier));
    let Some(modifier) = modifier else {
        eprintln!("no XRGB8888 layout is both renderable and sampleable; skipping");
        return None;
    };
    let gbm = gbm::Device::new(node).expect("a GBM device on the render node");
    Some((gpu_lock, renderer, gbm, modifier))
}

fn read(renderer: &mut Renderer, buffer: &Buffer, modifier: u64, x: u32, y: u32) -> [u8; 4] {
    let planes = buffer.planes();
    let texture = renderer
        .textures()
        .import_dmabuf(&spec(buffer, &planes, modifier))
        .expect("the dmabuf imports as a sampled texture");
    let mut target = renderer
        .create_target(buffer.width, buffer.height)
        .expect("an offscreen target");
    let mut scene = Scene::new();
    scene.set_background(Color::TRANSPARENT);
    scene.insert(Node::image(
        Rect::new(0.0, 0.0, buffer.width as f32, buffer.height as f32),
        texture,
    ));
    renderer
        .draw(&mut scene, &mut target)
        .expect("the imported buffer draws");
    let pixels = renderer.read_pixels(&mut target).expect("it reads back");
    let at = (y as usize * buffer.width as usize + x as usize) * 4;
    let got = [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]];
    renderer.textures().destroy(texture);
    got
}

fn ground() -> [u8; 4] {
    let [r, g, b, _] = GROUND.to_premultiplied();
    [
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        255,
    ]
}

fn assert_glass(got: [u8; 4], what: &str) {
    let want = ground();
    assert!(
        got.iter()
            .zip(want)
            .any(|(got, want)| got.abs_diff(want) > 8),
        "{what} came back {got:?}, which is the bare ground rather than glass over it"
    );
}

fn assert_ground(got: [u8; 4], what: &str) {
    let want = ground();
    assert!(
        got.iter()
            .zip(want)
            .all(|(got, want)| got.abs_diff(want) <= 2),
        "{what} came back {got:?}, not {want:?}"
    );
}

#[test]
fn scanout_glass_keeps_undamaged_pixels() {
    let Some((_gpu_lock, mut renderer, gbm, modifier)) = open("rikka-scanout-glass-test") else {
        return;
    };
    let display = Buffer::make(&gbm, modifier, WIDTH, HEIGHT);
    let mut output = renderer.create_dmabuf_output(DamageSource::CompleteForBuffer);
    let mut scene = Scene::immediate();
    {
        let planes = display.planes();
        let mut target = renderer
            .import_dmabuf(&spec(&display, &planes, modifier))
            .expect("the dmabuf imports as a render target");

        renderer.retain_backdrops(&[1]);
        frame(
            &mut scene,
            &[Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32)],
            true,
        );
        renderer
            .draw(&mut scene, output.with_target(&mut target))
            .expect("the first frame renders");

        for _ in 0..QUIET_FRAMES {
            renderer.retain_backdrops(&[1]);
            frame(&mut scene, &[CARET], false);
            renderer
                .draw(&mut scene, output.with_target(&mut target))
                .expect("a frame of damage renders");
        }

        renderer.retain_backdrops(&[1]);
        frame(&mut scene, &[PANE], true);
        renderer
            .draw(&mut scene, output.with_target(&mut target))
            .expect("the frame that redraws the glass renders");

        drop(target);
    }

    assert_ground(
        read(&mut renderer, &display, modifier, WIDTH - 8, HEIGHT - 8),
        "a corner nothing damaged",
    );
}

#[test]
fn toggling_glass_never_needs_full_redraw() {
    let Some((_gpu_lock, mut renderer, gbm, modifier)) = open("rikka-scanout-glassless-test")
    else {
        return;
    };
    let display = Buffer::make(&gbm, modifier, WIDTH, HEIGHT);
    let mut output = renderer.create_dmabuf_output(DamageSource::CompleteForBuffer);
    let mut scene = Scene::immediate();
    {
        let planes = display.planes();
        let mut target = renderer
            .import_dmabuf(&spec(&display, &planes, modifier))
            .expect("the dmabuf imports as a render target");

        let mut draw = |scene: &mut Scene, holds: &[u64], what: &str| {
            renderer.retain_backdrops(holds);
            let drawn = renderer
                .draw(scene, output.with_target(&mut target))
                .unwrap_or_else(|error| panic!("{what} does not render: {error}"));
            assert!(
                !drawn.output.needs_full_redraw,
                "{what} needs a full redraw"
            );
        };

        frame(&mut scene, &[whole()], true);
        draw(&mut scene, &[1], "the first frame");
        for _ in 0..4 {
            frame(&mut scene, &[CARET], false);
            draw(&mut scene, &[], "a frame with no glass on it");
        }
        frame(&mut scene, &[PANE], true);
        draw(&mut scene, &[1], "the frame the glass comes back on");

        drop(target);
    }

    assert_ground(
        read(&mut renderer, &display, modifier, WIDTH - 8, HEIGHT - 8),
        "a corner outside the pane",
    );
    assert_glass(
        read(&mut renderer, &display, modifier, 128, 64),
        "the middle of the pane",
    );
}
