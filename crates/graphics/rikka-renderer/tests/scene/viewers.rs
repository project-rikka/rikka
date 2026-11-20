use rikka_renderer::{Audience, Blur, Node, OffscreenTarget, Scene, Viewer};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Point, Rect};

const SIZE: u32 = 128;

const RED: (u8, u8, u8, u8) = (255, 0, 0, 255);
const BLUE: (u8, u8, u8, u8) = (0, 0, 255, 255);
const GREEN: (u8, u8, u8, u8) = (0, 255, 0, 255);
const WHITE: (u8, u8, u8, u8) = (255, 255, 255, 255);

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-test")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn seen_by(renderer: &mut TestRenderer, viewer: Viewer) -> OffscreenTarget {
    let mut target = renderer.create_target(SIZE, SIZE).unwrap();
    target.set_viewer(viewer);
    target
}

fn at(pixels: &[u8], spot: Point) -> (u8, u8, u8, u8) {
    let index = (spot.y as usize * SIZE as usize + spot.x as usize) * 4;
    (
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    )
}

fn square(x: f32, y: f32) -> Rect {
    Rect::new(x, y, 32.0, 32.0)
}

fn middle(rect: Rect) -> Point {
    rect.center()
}

#[test]
fn nodes_show_only_to_their_audience() {
    let mut renderer = renderer();
    for (made, kind) in [Scene::new, Scene::immediate]
        .into_iter()
        .zip(["retained", "immediate"])
    {
        let mut screen = seen_by(&mut renderer, Viewer::Screen);
        let mut capture = seen_by(&mut renderer, Viewer::Capture);
        let mut scene = made();
        scene.set_background(Color::rgb(1.0, 0.0, 0.0));
        let (screen_only, capture_only, everyone) =
            (square(8.0, 8.0), square(72.0, 8.0), square(8.0, 72.0));
        scene.insert(
            Node::rect(screen_only)
                .fill(Color::rgb(0.0, 0.0, 1.0))
                .audience(Audience::Only(Viewer::Screen)),
        );
        scene.insert(
            Node::rect(capture_only)
                .fill(Color::rgb(0.0, 1.0, 0.0))
                .audience(Audience::Only(Viewer::Capture)),
        );
        scene.insert(Node::rect(everyone).fill(Color::WHITE));

        renderer.draw(&mut scene, &mut screen).unwrap();
        renderer.draw(&mut scene, &mut capture).unwrap();
        let on_screen = renderer.read_pixels(&mut screen).unwrap();
        let captured = renderer.read_pixels(&mut capture).unwrap();

        let cases = [
            (&on_screen, middle(screen_only), BLUE),
            (&on_screen, middle(capture_only), RED),
            (&on_screen, middle(everyone), WHITE),
            (&captured, middle(screen_only), RED),
            (&captured, middle(capture_only), GREEN),
            (&captured, middle(everyone), WHITE),
        ];
        for (pixels, spot, want) in cases {
            assert_eq!(at(pixels, spot), want, "{kind} scene at {spot:?}");
        }
    }
}

#[test]
fn every_target_of_one_scene_catches_up() {
    let mut renderer = renderer();
    let mut screen = seen_by(&mut renderer, Viewer::Screen);
    let mut first = seen_by(&mut renderer, Viewer::Capture);
    let mut second = seen_by(&mut renderer, Viewer::Capture);
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    let (start, middle_stop, end) = (square(8.0, 8.0), square(48.0, 48.0), square(88.0, 88.0));
    let moving = scene.insert(Node::rect(start).fill(Color::WHITE));
    for target in [&mut screen, &mut first, &mut second] {
        renderer.draw(&mut scene, target).unwrap();
    }

    scene.edit(moving).rect(middle_stop);
    renderer.draw(&mut scene, &mut screen).unwrap();
    renderer.draw(&mut scene, &mut first).unwrap();
    scene.edit(moving).rect(end);
    for target in [&mut second, &mut first, &mut screen] {
        renderer.draw(&mut scene, target).unwrap();
    }

    for target in [&mut screen, &mut first, &mut second] {
        let pixels = renderer.read_pixels(target).unwrap();
        assert_eq!(at(&pixels, middle(start)), RED);
        assert_eq!(at(&pixels, middle(middle_stop)), RED);
        assert_eq!(at(&pixels, middle(end)), WHITE);
    }
}

#[test]
fn changes_hidden_from_a_viewer_leave_its_targets_undamaged() {
    let mut renderer = renderer();
    let mut screen = seen_by(&mut renderer, Viewer::Screen);
    let mut capture = seen_by(&mut renderer, Viewer::Capture);
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    let pointer = scene.insert(
        Node::rect(square(8.0, 8.0))
            .fill(Color::WHITE)
            .audience(Audience::Only(Viewer::Screen)),
    );
    renderer.draw(&mut scene, &mut screen).unwrap();
    renderer.draw(&mut scene, &mut capture).unwrap();

    scene.edit(pointer).rect(square(72.0, 72.0));
    assert!(renderer.draw(&mut scene, &mut screen).unwrap().rendered());
    assert!(!renderer.draw(&mut scene, &mut capture).unwrap().rendered());
}

#[test]
fn screen_only_node_behind_glass_never_leaks_into_capture() {
    let mut renderer = renderer();
    let mut screen = seen_by(&mut renderer, Viewer::Screen);
    let mut capture = seen_by(&mut renderer, Viewer::Capture);
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 1.0));
    scene.insert(
        Node::rect(Rect::new(16.0, 16.0, 96.0, 96.0))
            .fill(Color::rgb(1.0, 0.0, 0.0))
            .audience(Audience::Only(Viewer::Screen))
            .z(1),
    );
    scene.insert(
        Node::rounded_rect(Rect::new(16.0, 16.0, 96.0, 96.0), 8.0)
            .fill(Color::WHITE.with_alpha(0.1))
            .backdrop(Blur::new(6.0))
            .z(2),
    );
    renderer.draw(&mut scene, &mut screen).unwrap();
    renderer.draw(&mut scene, &mut capture).unwrap();

    let screen_px = renderer.read_pixels(&mut screen).unwrap();
    let capture_px = renderer.read_pixels(&mut capture).unwrap();
    let (sr, _, sb, _) = at(&screen_px, Point::new(64.0, 64.0));
    let (cr, _, cb, _) = at(&capture_px, Point::new(64.0, 64.0));
    assert!(
        sr > 200 && sb < 40,
        "screen sees red through glass: ({sr}, {sb})"
    );
    assert!(
        cr < 40 && cb > 200,
        "capture must not receive screen-only content through cached glass: ({cr}, {cb})"
    );
}
