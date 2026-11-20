use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rikka_gpu::{Pixels, TextureFormat};
use rikka_renderer::{Node, NodeId, Renderer, Scene, TextureId};
use rikka_text::TextSpec;
use rikka_types::{Color, Rect};

const FRAMES: usize = 40;
const VIDEO: [u32; 2] = [1280, 720];
const SIDE: u32 = 256;

fn renderer() -> (rikka_test_renderer::GpuLock, Renderer) {
    rikka_test_renderer::renderer("rikka-renderer-shared")
        .expect("a Vulkan 1.3 device is required for these tests")
        .split()
}

fn frame_of(shade: u8) -> Vec<u8> {
    let texel = [shade, 255 - shade, 64, 255];
    texel.repeat((VIDEO[0] * VIDEO[1]) as usize)
}

fn busy(scene: &mut Scene) -> Vec<NodeId> {
    (0..1200)
        .map(|index| {
            let rect = Rect::new(
                (index % 30) as f32 * 8.0,
                (index / 30) as f32 * 6.0,
                40.0,
                12.0,
            );
            scene.insert(
                Node::text(
                    rect,
                    TextSpec::new(format!("line {index}"), 9.0).layout(Some(rect.width())),
                )
                .z(index)
                .fill(Color::WHITE),
            )
        })
        .collect()
}

fn painting(
    renderer: &Arc<Mutex<Renderer>>,
    mut scene: Scene,
    edited: Vec<NodeId>,
    start: &Barrier,
) -> Vec<Duration> {
    let (mut target, mut preparer) = {
        let mut renderer = renderer.lock().unwrap();
        (
            renderer.create_target(SIDE, SIDE).unwrap(),
            renderer.preparer(),
        )
    };
    start.wait();
    let mut lock_waits = Vec::with_capacity(FRAMES);
    for frame in 0..FRAMES {
        for (at, id) in edited
            .iter()
            .enumerate()
            .filter(|(at, _)| at % 10 == frame % 10)
        {
            scene
                .edit(*id)
                .text(TextSpec::new(format!("{frame} {at}"), 9.0).layout(Some(40.0)));
        }
        preparer.prepare(&mut scene).unwrap();
        let mut renderer = renderer.lock().unwrap();
        let locked = Instant::now();
        renderer.draw(&mut scene, &mut target).unwrap();
        drop(renderer);
        lock_waits.push(locked.elapsed());
    }
    lock_waits
}

fn summary(waits: &mut [Duration]) -> String {
    waits.sort_unstable();
    let total: Duration = waits.iter().sum();
    let at = |share: f64| waits[((waits.len() - 1) as f64 * share) as usize];
    format!(
        "mean {:?}, p50 {:?}, p95 {:?}, max {:?}",
        total / waits.len() as u32,
        at(0.5),
        at(0.95),
        at(1.0)
    )
}

#[test]
fn video_upload_never_waits_on_other_surface() {
    let (_gpu_lock, renderer) = renderer();
    let uploads = renderer.uploads();
    let renderer = Arc::new(Mutex::new(renderer));
    let video: TextureId = uploads
        .create(Pixels::premultiplied(VIDEO[0], VIDEO[1], frame_of(0)))
        .unwrap();

    let mut shown = Scene::new();
    shown.set_background(Color::BLACK);
    shown.insert(Node::image(
        Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32),
        video,
    ));
    let captions = busy(&mut shown);
    let mut other = Scene::new();
    let lines = busy(&mut other);

    let start = Arc::new(Barrier::new(3));
    let first = {
        let (renderer, start) = (Arc::clone(&renderer), Arc::clone(&start));
        thread::spawn(move || painting(&renderer, shown, captions, &start))
    };
    let second = {
        let (renderer, start) = (Arc::clone(&renderer), Arc::clone(&start));
        thread::spawn(move || painting(&renderer, other, lines, &start))
    };

    start.wait();
    let mut ui = Vec::with_capacity(FRAMES);
    let mut last = 0;
    for frame in 0..FRAMES {
        last = (frame * 2 % 256) as u8;
        let pixels = frame_of(last);
        let started = Instant::now();
        uploads
            .update(video, TextureFormat::Rgba8Premultiplied, pixels)
            .unwrap();
        ui.push(started.elapsed());
        thread::sleep(Duration::from_millis(4));
    }
    let mut lock_waits = first.join().unwrap();
    lock_waits.extend(second.join().unwrap());

    let mut renderer = Arc::into_inner(renderer).unwrap().into_inner().unwrap();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut check = Scene::new();
    check.insert(Node::image(
        Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32),
        video,
    ));
    renderer.draw(&mut check, &mut target).unwrap();
    let pixels = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(
        &pixels[..3],
        &[last, 255 - last, 64],
        "the last frame the UI thread handed over is what is shown"
    );

    let p95 = ui[(ui.len() - 1) * 95 / 100];
    assert!(
        p95 < Duration::from_millis(1),
        "handing a video frame over waited {p95:?}: the UI thread was blocked {}, while the renderer was held per frame {}",
        summary(&mut ui),
        summary(&mut lock_waits)
    );
}
