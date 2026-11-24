mod host;

use rikka_renderer::{Blur, Error, ExternalImage, Node, Renderer, Scene, Transform};
use rikka_types::{Color, Rect};

use host::{Host, SIDE};

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * SIDE + x) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

fn transparent_scene() -> Scene {
    let mut scene = Scene::new();
    scene.set_background(Color::TRANSPARENT);
    scene
}

#[test]
fn external_draw_keeps_the_host_image_beneath_the_scene() {
    let _gpu = rikka_test_renderer::lock_gpu();
    let host = Host::new();
    host.paint_halves(GREEN, GREEN);
    let mut renderer = Renderer::new(
        &rikka_gpu::Gpu::builder()
            .external(host.device())
            .build()
            .unwrap(),
    )
    .unwrap();
    let mut target = renderer.create_external_target();
    let mut scene = transparent_scene();
    let half = SIDE as f32 / 2.0;
    scene
        .insert(Node::rect(Rect::new(0.0, 0.0, half, SIDE as f32)).fill(Color::rgb(1.0, 0.0, 0.0)));

    renderer
        .draw(&mut scene, unsafe { target.onto(host.image()) })
        .unwrap();
    let pixels = host.read();

    assert_eq!(pixel(&pixels, 4, 8), [255, 0, 0, 255], "the scene draws");
    assert_eq!(
        pixel(&pixels, SIDE - 4, 8),
        [0, 255, 0, 255],
        "the host image stays beside it"
    );

    host.paint_halves(GREEN, GREEN);
    renderer
        .draw(&mut scene, unsafe { target.onto(host.image()) })
        .unwrap();
    let again = host.read();
    assert_eq!(
        pixel(&again, 4, 8),
        [255, 0, 0, 255],
        "an unchanged scene draws again over a new frame"
    );
}

#[test]
fn external_draw_follows_the_host_image_orientation() {
    let _gpu = rikka_test_renderer::lock_gpu();
    let host = Host::new();
    host.paint_halves(GREEN, GREEN);
    let mut renderer = Renderer::new(
        &rikka_gpu::Gpu::builder()
            .external(host.device())
            .build()
            .unwrap(),
    )
    .unwrap();
    let mut target = renderer.create_external_target();
    let mut scene = transparent_scene();
    let half = SIDE as f32 / 2.0;
    scene
        .insert(Node::rect(Rect::new(0.0, 0.0, SIDE as f32, half)).fill(Color::rgb(1.0, 0.0, 0.0)));
    let bottom_up = ExternalImage {
        transform: Transform::Flipped180,
        ..host.image()
    };

    renderer
        .draw(&mut scene, unsafe { target.onto(bottom_up) })
        .unwrap();
    let pixels = host.read();

    assert_eq!(
        pixel(&pixels, 8, SIDE - 4),
        [255, 0, 0, 255],
        "the scene's top lands on the last rows"
    );
    assert_eq!(
        pixel(&pixels, 8, 4),
        [0, 255, 0, 255],
        "the first rows keep the host image"
    );
}

#[test]
fn external_backdrops_blur_the_host_image() {
    let _gpu = rikka_test_renderer::lock_gpu();
    let host = Host::new();
    host.paint_halves(BLACK, WHITE);
    let mut renderer = Renderer::new(
        &rikka_gpu::Gpu::builder()
            .external(host.device())
            .build()
            .unwrap(),
    )
    .unwrap();
    let mut target = renderer.create_external_target();
    let mut scene = transparent_scene();
    let whole = Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32);
    scene.insert(
        Node::rect(whole)
            .fill(Color::TRANSPARENT)
            .backdrop(Blur::with_radius(8.0)),
    );

    renderer
        .draw(&mut scene, unsafe { target.onto(host.image()) })
        .unwrap();
    let pixels = host.read();

    let [seam, ..] = pixel(&pixels, SIDE / 2, SIDE / 2);
    assert!(
        (32..224).contains(&seam),
        "the seam between the host's halves is blurred, not {seam}"
    );
}

#[test]
fn external_device_without_the_needed_features_is_refused() {
    let _gpu = rikka_test_renderer::lock_gpu();
    let host = Host::new();
    let refused = rikka_gpu::Gpu::builder()
        .external(host.bare_device())
        .build()
        .and_then(|gpu| Renderer::new(&gpu));
    assert!(
        matches!(refused, Err(Error::Unsupported(_))),
        "a device created without descriptor indexing cannot be drawn with"
    );
}
