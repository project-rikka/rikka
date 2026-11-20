use rikka_renderer::{Node, NodeId, OffscreenTarget, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Rect};

const SIDE: u32 = 64;
const PAST_THE_CACHE: usize = 250;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-scene-cache")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn centre(renderer: &mut TestRenderer, target: &mut OffscreenTarget) -> (u8, u8, u8, u8) {
    let pixels = renderer.read_pixels(target).unwrap();
    let at = ((SIDE / 2 * SIDE + SIDE / 2) * 4) as usize;
    (pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3])
}

fn filled(color: Color) -> (Scene, NodeId) {
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let node = scene.insert(Node::rect(Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32)).fill(color));
    (scene, node)
}

#[test]
fn expired_scene_cache_redraws_changes() {
    let mut renderer = renderer();
    let mut shown = renderer.create_target(SIDE, SIDE).unwrap();
    let mut aside = renderer.create_target(SIDE, SIDE).unwrap();
    let (mut a, a_node) = filled(Color::WHITE);
    let (mut b, b_node) = filled(Color::rgb(1.0, 0.0, 0.0));

    renderer.draw(&mut a, &mut shown).unwrap();
    for color in [
        Color::rgb(0.0, 1.0, 0.0),
        Color::rgb(0.0, 0.0, 1.0),
        Color::rgb(1.0, 0.0, 0.0),
    ] {
        b.edit(b_node).fill(color);
        renderer.draw(&mut b, &mut aside).unwrap();
        centre(&mut renderer, &mut aside);
    }
    assert_eq!(centre(&mut renderer, &mut aside), (255, 0, 0, 255));

    for _ in 0..PAST_THE_CACHE {
        centre(&mut renderer, &mut shown);
    }
    b.edit(b_node).fill(Color::rgb(0.0, 1.0, 0.0));
    renderer.draw(&mut b, &mut aside).unwrap();
    assert_eq!(
        centre(&mut renderer, &mut aside),
        (0, 255, 0, 255),
        "the change made after the scene was forgotten"
    );

    for frame in 0..PAST_THE_CACHE {
        let shade = if frame % 2 == 0 {
            Color::rgb(1.0, 1.0, 0.0)
        } else {
            Color::WHITE
        };
        a.edit(a_node).fill(shade);
        renderer.draw(&mut a, &mut shown).unwrap();
    }
    b.edit(b_node).fill(Color::rgb(0.0, 0.0, 1.0));
    renderer.draw(&mut b, &mut aside).unwrap();
    assert_eq!(
        centre(&mut renderer, &mut aside),
        (0, 0, 255, 255),
        "the change made while only the other scene was drawn"
    );
    assert_eq!(centre(&mut renderer, &mut shown), (255, 255, 255, 255));
}
