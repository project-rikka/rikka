use rikka_gpu::Pixels;
use rikka_renderer::{
    BackdropCache, Blur, Node, NodeId, OffscreenTarget, Scene, Stroke, TextureId,
};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, Lens, Rect};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 224;
const WHOLE: Rect = Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
const PANE: Rect = Rect::new(150.0, 70.0, 150.0, 130.0);
const STRIPE: Rect = Rect::new(60.0, 60.0, 40.0, 90.0);
const OPAQUE_WINDOW: Rect = Rect::new(20.0, 120.0, 140.0, 96.0);

#[derive(Clone, Copy)]
struct Desk {
    ground: TextureId,
    stripe: Color,
    card_x: f32,
    card_shown: bool,
    cache: u64,
}

struct Placed {
    stripe: NodeId,
    card: NodeId,
    pane: NodeId,
}

type Step = (&'static str, fn(&mut Desk));

fn card(x: f32) -> Rect {
    Rect::new(x, 30.0, 180.0, 140.0)
}

fn ground() -> Pixels {
    let texels = (0..WIDTH * HEIGHT).flat_map(|at| {
        let (x, y) = (at % WIDTH, at / WIDTH);
        [(x * 255 / WIDTH) as u8, 40, (y * 255 / HEIGHT) as u8, 255]
    });
    Pixels::premultiplied(WIDTH, HEIGHT, texels.collect::<Vec<u8>>())
}

fn lay(scene: &mut Scene, desk: Desk) -> Placed {
    scene.set_background(Color::rgb(0.05, 0.05, 0.08));
    scene.insert(Node::image(WHOLE, desk.ground));
    for band in 0..8 {
        let x = 16.0 + band as f32 * 36.0;
        scene.insert(
            Node::rect(Rect::new(x, 0.0, 18.0, HEIGHT as f32)).fill(Color::rgb(
                0.1,
                0.3 + band as f32 * 0.08,
                0.9,
            )),
        );
    }
    scene.insert(Node::rect(Rect::new(230.0, 120.0, 60.0, 60.0)).fill(Color::WHITE));
    let stripe = scene.insert(Node::rect(STRIPE).fill(desk.stripe));
    let card = scene.insert(
        Node::rounded_rect(card(desk.card_x), 14.0)
            .fill(Color::rgb(0.97, 0.97, 0.95))
            .stroke(Stroke::new(3.0, Color::rgb(0.2, 0.2, 0.25).with_alpha(0.6)))
            .visible(desk.card_shown),
    );
    scene.insert(
        Node::image(OPAQUE_WINDOW, desk.ground)
            .radii(10.0)
            .opaque_within(OPAQUE_WINDOW),
    );
    scene.insert(
        Node::rounded_rect(Rect::new(70.0, 80.0, 60.0, 40.0), 6.0)
            .fill(Color::rgb(0.2, 0.7, 0.3).with_alpha(0.5)),
    );
    let pane = scene.insert(
        Node::rounded_rect(PANE, 18.0)
            .fill(Color::WHITE.with_alpha(0.2))
            .backdrop(Blur::with_radius(16.0))
            .backdrop_cache(BackdropCache::current(desk.cache))
            .lens(Lens::new().scaled(0.25)),
    );
    scene.insert(
        Node::rounded_rect(Rect::new(210.0, 100.0, 100.0, 100.0), 8.0)
            .fill(Color::rgb(0.95, 0.9, 0.2)),
    );
    Placed { stripe, card, pane }
}

fn fresh(renderer: &mut TestRenderer, target: &mut OffscreenTarget, desk: Desk) -> Vec<u8> {
    let mut scene = Scene::immediate();
    lay(&mut scene, desk);
    renderer.draw(&mut scene, &mut *target).unwrap();
    renderer.read_pixels(target).unwrap()
}

fn assert_same(step: &str, got: &[u8], want: &[u8]) {
    for (at, (got, want)) in got
        .as_chunks::<4>()
        .0
        .iter()
        .zip(want.as_chunks::<4>().0)
        .enumerate()
    {
        let off = got.iter().zip(want).map(|(a, b)| a.abs_diff(*b)).max();
        assert!(
            off.is_some_and(|off| off <= 2),
            "{step}: pixel ({}, {}) came back {got:?} where a fresh draw gives {want:?}",
            at % WIDTH as usize,
            at / WIDTH as usize,
        );
    }
}

#[test]
fn occluded_content_matches_fresh_draw() {
    let mut renderer = rikka_test_renderer::renderer("rikka-renderer-occlusion")
        .expect("a Vulkan 1.3 device is required for these tests");
    let mut target = renderer.create_target(WIDTH, HEIGHT).unwrap();
    let mut whole = renderer.create_target(WIDTH, HEIGHT).unwrap();
    let ground = renderer.textures().create(ground()).unwrap();
    let mut desk = Desk {
        ground,
        stripe: Color::rgb(0.9, 0.9, 0.1),
        card_x: 40.0,
        card_shown: true,
        cache: 1,
    };
    let mut scene = Scene::new();
    let placed = lay(&mut scene, desk);
    let mut key = 10;

    let steps: [Step; 5] = [
        ("first frame", |_| {}),
        ("a change beneath the card", |desk| {
            desk.stripe = Color::rgb(0.1, 0.9, 0.4)
        }),
        ("the card moved aside", |desk| desk.card_x = 110.0),
        ("the card hidden", |desk| desk.card_shown = false),
        ("the card back over the stripe", |desk| {
            desk.card_shown = true;
            desk.card_x = 30.0;
            desk.stripe = Color::rgb(0.6, 0.1, 0.7);
        }),
    ];
    for (step, change) in steps {
        let was = desk;
        change(&mut desk);
        if desk.stripe != was.stripe {
            scene.edit(placed.stripe).fill(desk.stripe);
        }
        if desk.card_x != was.card_x {
            scene.edit(placed.card).rect(card(desk.card_x));
        }
        if desk.card_shown != was.card_shown {
            scene.edit(placed.card).visible(desk.card_shown);
        }
        scene
            .edit(placed.pane)
            .backdrop_cache(BackdropCache::stale(desk.cache, [WHOLE]));
        renderer.retain_backdrops(&[desk.cache]);
        renderer.draw(&mut scene, &mut target).unwrap();
        scene
            .edit(placed.pane)
            .backdrop_cache(BackdropCache::current(desk.cache));
        scene.damage_all();
        renderer.retain_backdrops(&[desk.cache]);
        renderer.draw(&mut scene, &mut target).unwrap();
        let got = renderer.read_pixels(&mut target).unwrap();

        key += 1;
        renderer.retain_backdrops(&[desk.cache, key]);
        let want = fresh(&mut renderer, &mut whole, Desk { cache: key, ..desk });
        assert_same(step, &got, &want);
    }
}

#[test]
fn occlusion_handles_large_instance_counts_and_stale_textures() {
    let mut renderer = rikka_test_renderer::renderer("rikka-renderer-test")
        .expect("a Vulkan 1.3 device is required for these tests");
    let mut target = renderer.create_target(WIDTH, HEIGHT).unwrap();
    let mut reference = renderer.create_target(WIDTH, HEIGHT).unwrap();

    let live_tex = renderer
        .textures()
        .create(Pixels::premultiplied(64, 64, vec![255u8; 64 * 64 * 4]))
        .unwrap();
    let stale_tex = renderer
        .textures()
        .create(Pixels::premultiplied(64, 64, vec![128u8; 64 * 64 * 4]))
        .unwrap();

    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.1, 0.2, 0.8));
    for index in 0..320 {
        let x = (index % 32) as f32 * 10.0;
        let y = (index / 32) as f32 * 10.0;
        scene.insert(
            Node::rect(Rect::new(x, y, 8.0, 8.0))
                .fill(Color::rgb(0.2, 0.8, 0.2))
                .z(1),
        );
    }
    scene.insert(
        Node::image(Rect::new(16.0, 16.0, 128.0, 128.0), live_tex)
            .opaque_within(Rect::new(16.0, 16.0, 128.0, 128.0))
            .z(2),
    );
    let stale_node = scene.insert(
        Node::image(Rect::new(120.0, 40.0, 128.0, 128.0), stale_tex)
            .opaque_within(Rect::new(120.0, 40.0, 128.0, 128.0))
            .z(3),
    );
    renderer.draw(&mut scene, &mut target).unwrap();

    renderer.textures().destroy(stale_tex);
    scene.edit(stale_node).opacity(1.0);
    scene.damage_all();
    renderer.draw(&mut scene, &mut target).unwrap();
    let got = renderer.read_pixels(&mut target).unwrap();

    let mut fresh = Scene::new();
    fresh.set_background(Color::rgb(0.1, 0.2, 0.8));
    for index in 0..320 {
        let x = (index % 32) as f32 * 10.0;
        let y = (index / 32) as f32 * 10.0;
        fresh.insert(
            Node::rect(Rect::new(x, y, 8.0, 8.0))
                .fill(Color::rgb(0.2, 0.8, 0.2))
                .z(1),
        );
    }
    fresh.insert(
        Node::image(Rect::new(16.0, 16.0, 128.0, 128.0), live_tex)
            .opaque_within(Rect::new(16.0, 16.0, 128.0, 128.0))
            .z(2),
    );
    renderer.draw(&mut fresh, &mut reference).unwrap();
    let want = renderer.read_pixels(&mut reference).unwrap();

    assert_same("stale texture must not occlude background", &got, &want);
    renderer.textures().destroy(live_tex);
}
