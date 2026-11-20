use rikka_renderer::{Node, Scene, TexelRect};
use rikka_types::{Color, Rect};

const SIDE: u32 = 16;

fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * SIDE + x) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

#[test]
fn readback_areas_refresh_only_those_areas() {
    let mut renderer = rikka_test_renderer::renderer("rikka-renderer-readback")
        .expect("a Vulkan 1.3 device is required for these tests");
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let whole = Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32);
    let fill = scene.insert(Node::rect(whole).fill(Color::rgb(1.0, 0.0, 0.0)));
    renderer.draw(&mut scene, &mut target).unwrap();
    let mut readback = renderer.create_readback();
    renderer.start_readback(&mut target, &mut readback).unwrap();
    assert!(renderer.wait_for_pixels(&mut readback).unwrap());

    scene.edit(fill).fill(Color::rgb(0.0, 0.0, 1.0));
    renderer.draw(&mut scene, &mut target).unwrap();
    renderer
        .start_readback_of(
            &mut target,
            &mut readback,
            &[TexelRect::new([4, 2], [6, 3])],
        )
        .unwrap();
    assert!(renderer.wait_for_pixels(&mut readback).unwrap());

    let pixels = readback.pixels();
    assert_eq!(pixels.len(), (SIDE * SIDE * 4) as usize);
    assert_eq!(
        pixel(pixels, 4, 2),
        [0, 0, 255, 255],
        "the area is read again"
    );
    assert_eq!(pixel(pixels, 9, 4), [0, 0, 255, 255], "to its far corner");
    assert_eq!(
        pixel(pixels, 10, 4),
        [255, 0, 0, 255],
        "beside it the old frame stays"
    );
    assert_eq!(pixel(pixels, 4, 5), [255, 0, 0, 255], "and below it");
}

#[test]
fn copied_draw_copies_its_damage_and_the_missed_areas() {
    let mut renderer = rikka_test_renderer::renderer("rikka-renderer-copied-draw")
        .expect("a Vulkan 1.3 device is required for these tests");
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let half = SIDE as f32 / 2.0;
    let red = Color::rgb(1.0, 0.0, 0.0);
    let left = scene.insert(Node::rect(Rect::new(0.0, 0.0, half, SIDE as f32)).fill(red));
    let right = scene.insert(Node::rect(Rect::new(half, 0.0, half, SIDE as f32)).fill(red));
    let mut readback = renderer.create_readback();
    renderer
        .draw(&mut scene, target.copied_into(&mut readback, &[]))
        .unwrap();
    assert!(renderer.wait_for_pixels(&mut readback).unwrap());
    assert_eq!(pixel(readback.pixels(), 4, 4), [255, 0, 0, 255]);

    scene.edit(left).fill(Color::rgb(0.0, 0.0, 1.0));
    renderer.draw(&mut scene, &mut target).unwrap();
    scene.edit(right).fill(Color::rgb(0.0, 1.0, 0.0));
    let missed = [TexelRect::new([0, 0], [4, SIDE])];
    renderer
        .draw(&mut scene, target.copied_into(&mut readback, &missed))
        .unwrap();
    assert!(renderer.wait_for_pixels(&mut readback).unwrap());

    let pixels = readback.pixels();
    assert_eq!(
        pixel(pixels, 12, 8),
        [0, 255, 0, 255],
        "the damage is copied"
    );
    assert_eq!(pixel(pixels, 1, 8), [0, 0, 255, 255], "the missed area too");
    assert_eq!(pixel(pixels, 6, 8), [255, 0, 0, 255], "nothing else");
}
