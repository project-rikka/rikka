use std::f32::consts::FRAC_PI_2;

use rikka_renderer::{Node, OffscreenTarget, Scene, Stroke};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Gradient, Rect, RoundedRect};

const SIDE: u32 = 64;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-scene-paints")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn pixel(renderer: &mut TestRenderer, target: &mut OffscreenTarget, x: u32, y: u32) -> [u8; 4] {
    let pixels = renderer.read_pixels(target).unwrap();
    let at = ((y * SIDE + x) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

fn left_to_right() -> Gradient {
    Gradient::linear(
        FRAC_PI_2,
        Color::rgb(1.0, 0.0, 0.0),
        Color::rgb(0.0, 0.0, 1.0),
    )
}

#[test]
fn fill_gradient_runs_along_its_angle_and_follows_moves() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let node = scene.insert(
        Node::rect(Rect::new(0.0, 0.0, 32.0, SIDE as f32))
            .fill(Color::WHITE)
            .fill_gradient(left_to_right()),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    let left = pixel(&mut renderer, &mut target, 0, 32);
    let right = pixel(&mut renderer, &mut target, 31, 32);
    assert!(left[0] > 240 && left[2] < 16, "left edge {left:?}");
    assert!(right[2] > 240 && right[0] < 16, "right edge {right:?}");

    scene
        .edit(node)
        .rect(Rect::new(32.0, 0.0, 32.0, SIDE as f32));
    renderer.draw(&mut scene, &mut target).unwrap();
    let moved = pixel(&mut renderer, &mut target, 32, 32);
    assert!(moved[0] > 240 && moved[2] < 16, "moved left edge {moved:?}");
    assert_eq!(pixel(&mut renderer, &mut target, 8, 32), [0, 0, 0, 255]);

    scene.edit(node).fill_gradient(None);
    renderer.draw(&mut scene, &mut target).unwrap();
    assert_eq!(
        pixel(&mut renderer, &mut target, 48, 32),
        [255, 255, 255, 255]
    );
}

#[test]
fn stroke_gradient_paints_only_the_rim() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(
        Node::rect(Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32))
            .fill(Color::TRANSPARENT)
            .stroke(Stroke::new(4.0, Color::WHITE))
            .stroke_gradient(left_to_right()),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    let rim = pixel(&mut renderer, &mut target, 1, 32);
    assert!(rim[0] > 240 && rim[2] < 16, "left rim {rim:?}");
    let rim = pixel(&mut renderer, &mut target, 62, 32);
    assert!(rim[2] > 240 && rim[0] < 16, "right rim {rim:?}");
    assert_eq!(pixel(&mut renderer, &mut target, 32, 32), [0, 0, 0, 255]);
}

#[test]
fn shadow_fades_outward_and_spares_its_cut_out_shape() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let shape = RoundedRect::new(Rect::new(20.0, 20.0, 24.0, 24.0), 4.0);
    scene.insert(Node::shadow(shape, 4.0).fill(Color::WHITE).cut_out(shape));
    renderer.draw(&mut scene, &mut target).unwrap();

    assert_eq!(pixel(&mut renderer, &mut target, 32, 32), [0, 0, 0, 255]);
    let near = pixel(&mut renderer, &mut target, 18, 32)[0];
    let far = pixel(&mut renderer, &mut target, 12, 32)[0];
    let outside = pixel(&mut renderer, &mut target, 2, 32)[0];
    assert!(near > far, "near {near} is not brighter than far {far}");
    assert!(near > 64 && near < 160, "edge shade {near}");
    assert_eq!(outside, 0);
}
