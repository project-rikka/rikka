use rikka_renderer::{Node, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_text::TextSpec;
use rikka_types::{Color, Rect};

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-glyph-atlas")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn lit(pixels: &[u8]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 32 || p[1] > 32 || p[2] > 32)
        .count()
}

#[test]
fn oversized_glyph_falls_back_to_curves() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(512, 128).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(
        Node::text(
            Rect::new(0.0, 0.0, 9000.0, 9000.0),
            TextSpec::new("W", 6000.0).layout(Some(9000.0)),
        )
        .fill(Color::WHITE),
    );
    scene.insert(
        Node::text(
            Rect::new(8.0, 8.0, 496.0, 112.0),
            TextSpec::new("Rikka renderer", 24.0).layout(Some(496.0)),
        )
        .fill(Color::WHITE),
    );

    renderer.draw(&mut scene, &mut target).unwrap();
    let first = lit(&renderer.read_pixels(&mut target).unwrap());
    assert!(
        first > 200,
        "expected glyph coverage, found {first} lit pixels"
    );

    scene.damage_all();
    renderer.draw(&mut scene, &mut target).unwrap();
    let again = lit(&renderer.read_pixels(&mut target).unwrap());
    assert_eq!(again, first, "the frame after draws the same text");
}
