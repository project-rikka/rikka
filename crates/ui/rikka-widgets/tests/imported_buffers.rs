use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use rikka_picture::{ImportedFrame, ImportedPlane, Lease, Tick};
use rikka_renderer::Scene;
use rikka_theme::Theme;
use rikka_types::{Color, Size};
use rikka_ui::*;
use rikka_ui_host::Resources;
use rikka_widgets::*;

const VIEW: u32 = 64;
const UNCOVERED: Color = Color::rgb(1.0, 0.0, 0.0);

fn pixel(pixels: &[u8], x: usize, y: usize) -> (u8, u8, u8, u8) {
    let i = (y * VIEW as usize + x) * 4;
    (pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3])
}

#[derive(State)]
struct Handed {
    picture: ImportedBuffer,
}

fn handed_view(handed: &Handed) -> Element {
    image(ImageSource::Imported(handed.picture.clone()))
        .width(Length::px(VIEW as f32))
        .height(Length::px(VIEW as f32))
        .build()
}

const RENDER_NODE: &str = "/dev/dri/renderD128";

const XRGB8888: u32 = gbm::Format::Xrgb8888 as u32;

const HANDED_FILL: u32 = 0x3366cc;

#[test]
fn imported_buffer_draws_and_imports_once() {
    use std::os::fd::{AsFd, OwnedFd};
    use std::os::unix::fs::MetadataExt;

    use rikka_gpu::DmabufPlane;
    use rikka_renderer::{DamageSource, DmabufSpec, Node};
    use rikka_types::Rect;

    if !std::path::Path::new(RENDER_NODE).exists() {
        eprintln!("{RENDER_NODE} is absent; skipping");
        return;
    }
    let node = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(RENDER_NODE)
        .expect("failed to open render node");
    let device = node.metadata().expect("render node metadata").rdev();

    let _gpu_lock = rikka_test_renderer::lock_gpu();
    let mut renderer = rikka_renderer::Renderer::new(
        &rikka_gpu::Gpu::builder()
            .app_name("rikka-ui-handed-test")
            .drm_node(libc::major(device), libc::minor(device))
            .build()
            .expect("no Vulkan device for render node"),
    )
    .expect("no Vulkan device for render node");

    let sampleable: Vec<u64> = renderer
        .gpu()
        .dmabuf_texture_formats()
        .iter()
        .filter(|format| format.fourcc == XRGB8888)
        .map(|format| format.modifier)
        .collect();
    let Some(modifier) = renderer
        .gpu()
        .dmabuf_render_formats()
        .iter()
        .filter(|format| format.fourcc == XRGB8888)
        .map(|format| format.modifier)
        .find(|modifier| sampleable.contains(modifier))
    else {
        eprintln!("no XRGB8888 layout is both renderable and sampleable; skipping");
        return;
    };

    let gbm = gbm::Device::new(node).expect("GBM device on render node");
    let bo = gbm
        .create_buffer_object_with_modifiers2::<()>(
            VIEW,
            VIEW,
            gbm::Format::Xrgb8888,
            [gbm::Modifier::from(modifier)].into_iter(),
            gbm::BufferObjectFlags::RENDERING,
        )
        .expect("renderable and sampleable buffer");
    let fds: Vec<OwnedFd> = (0..bo.plane_count() as i32)
        .map(|plane| bo.fd_for_plane(plane).expect("failed to export dmabuf"))
        .collect();
    let planes: Vec<DmabufPlane<'_>> = fds
        .iter()
        .enumerate()
        .map(|(index, fd)| DmabufPlane {
            offset: bo.offset(index as i32),
            stride: bo.stride_for_plane(index as i32),
            fd: fd.as_fd(),
        })
        .collect();

    {
        let mut buffer = renderer
            .import_dmabuf(&DmabufSpec {
                width: VIEW,
                height: VIEW,
                fourcc: XRGB8888,
                modifier,
                planes: &planes,
            })
            .expect("failed to import dmabuf as render target");
        let mut painted = Scene::new();
        painted.set_background(Color::hex(HANDED_FILL));
        painted.insert(
            Node::rect(Rect::new(0.0, 0.0, VIEW as f32, VIEW as f32)).fill(Color::hex(HANDED_FILL)),
        );
        let mut output = renderer.create_dmabuf_output(DamageSource::SinceLastFrame);
        renderer
            .draw(&mut painted, output.with_target(&mut buffer))
            .expect("failed to draw into buffer");
        drop(buffer);
    }

    let picture = ImportedBuffer::new();
    let arrival = |buffer: u64| ImportedFrame {
        buffer,
        width: VIEW,
        height: VIEW,
        fourcc: XRGB8888,
        modifier,
        planes: fds
            .iter()
            .enumerate()
            .map(|(index, fd)| ImportedPlane {
                fd: fd
                    .as_fd()
                    .try_clone_to_owned()
                    .expect("failed to duplicate fd"),
                offset: bo.offset(index as i32),
                stride: bo.stride_for_plane(index as i32),
            })
            .collect(),
        lease: None,
        pulse: None,
        damage: None,
    };

    let mut target = renderer.create_target(VIEW, VIEW).unwrap();
    let mut scene = Scene::new();
    let mut resources = Resources::new();
    let now = Instant::now();
    let mut ui = Ui::new(
        Handed {
            picture: picture.clone(),
        },
        handed_view,
        Theme::dark(),
    );
    ui.set_background(Some(UNCOVERED));
    ui.set_viewport(Size::new(VIEW as f32, VIEW as f32), 1.0);
    ui.render(
        Rendering::new(&mut scene)
            .with_time(now)
            .resolving(&mut resources),
    );

    let mut turn = |resources: &mut Resources| {
        let frame = resources.commit(renderer.as_ref());
        for texture in &frame.updated {
            scene.notify_texture_updated(*texture);
        }
        for (_, pulse) in &frame.pulsed {
            pulse.beat(Tick {
                at: now,
                period: Duration::ZERO,
            });
        }
        ui.images_arrived(&frame.settled, !frame.updated.is_empty() || frame.repainted);
        ui.render(
            Rendering::new(&mut scene)
                .with_time(now)
                .resolving(resources),
        );
        let output = renderer.draw(&mut scene, &mut target).unwrap();
        let pixels = renderer.read_pixels(&mut target).unwrap();
        let redrawn = output
            .damage
            .iter()
            .copied()
            .reduce(|joined, area| joined.union(&area));
        (
            pixel(&pixels, 32, 32),
            frame.settled.len(),
            frame.updated.len(),
            redrawn,
        )
    };

    picture.show(arrival(1));
    let (drawn, imported, updated, _) = turn(&mut resources);
    assert_eq!(imported, 1, "buffer not imported");
    assert_eq!(updated, 1, "reported as updated");
    let expected = (
        ((HANDED_FILL >> 16) & 0xff) as u8,
        ((HANDED_FILL >> 8) & 0xff) as u8,
        (HANDED_FILL & 0xff) as u8,
    );
    assert!(
        drawn.0.abs_diff(expected.0) <= 1
            && drawn.1.abs_diff(expected.1) <= 1
            && drawn.2.abs_diff(expected.2) <= 1,
        "window image mismatch: {drawn:?} vs {expected:?}"
    );

    picture.show(arrival(1));
    let (again, imported, updated, _) = turn(&mut resources);
    assert_eq!(imported, 0, "buffer imported twice");
    assert_eq!(updated, 1, "update without import");
    assert_eq!(again, drawn, "same image drawn");

    let released = Arc::new(AtomicUsize::new(0));
    let beaten = Arc::new(AtomicUsize::new(0));
    let leased = |buffer: u64| {
        let released = Arc::clone(&released);
        let beaten = Arc::clone(&beaten);
        ImportedFrame {
            lease: Some(Lease::new().on_released(move || {
                released.fetch_add(1, Ordering::SeqCst);
            })),
            pulse: Some(Pulse::new(move |_| {
                beaten.fetch_add(1, Ordering::SeqCst);
            })),
            ..arrival(buffer)
        }
    };

    picture.show(leased(2));
    turn(&mut resources);
    assert_eq!(
        beaten.load(Ordering::SeqCst),
        1,
        "arrival commit returned no pulse"
    );
    turn(&mut resources);
    assert_eq!(beaten.load(Ordering::SeqCst), 1, "only one buffer imported");
    assert_eq!(
        released.load(Ordering::SeqCst),
        0,
        "displayed buffer released early"
    );

    for _ in 0..4 {
        picture.show(arrival(3));
        turn(&mut resources);
    }
    assert_eq!(
        released.load(Ordering::SeqCst),
        1,
        "replaced buffer not released"
    );

    let corner = || Some(vec![Rect::new(8.0, 8.0, 4.0, 4.0)]);
    for (buffer, how) in [(3, "the same buffer drawn again"), (4, "a fresh buffer")] {
        picture.show(ImportedFrame {
            damage: corner(),
            ..arrival(buffer)
        });
        let (shown, _, updated, redrawn) = turn(&mut resources);
        assert_eq!(updated, 0, "{how}: partial update reported as full");
        let redrawn = redrawn.expect("damaged corner not redrawn");
        assert!(
            redrawn.min_x() >= 4.0 && redrawn.max_x() <= 16.0 && redrawn.max_y() <= 16.0,
            "{how}: redraw exceeded damaged corner: {redrawn:?}"
        );
        assert_eq!(shown, drawn, "{how}: image corrupted");
    }

    picture.show(arrival(3));
    let (_, _, updated, redrawn) = turn(&mut resources);
    assert_eq!(updated, 1, "undamaged buffer counts as full update");
    assert!(redrawn.is_some_and(|redrawn| redrawn.width() >= VIEW as f32));
}
