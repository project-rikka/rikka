use std::time::Instant;

use rikka_renderer::{FrameWork, Node, NodeId, OffscreenTarget, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_text::TextSpec;
use rikka_types::{Color, Point, Rect};

const SIDE: u32 = 64;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-frame-work")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn pixel(renderer: &mut TestRenderer, target: &mut OffscreenTarget, x: u32, y: u32) -> [u8; 4] {
    let pixels = renderer.read_pixels(target).unwrap();
    let at = ((y * SIDE + x) * 4) as usize;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

fn lit(pixels: &[u8]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 32 || p[1] > 32 || p[2] > 32)
        .count()
}

fn drawn(
    renderer: &mut TestRenderer,
    scene: &mut Scene,
    target: &mut OffscreenTarget,
) -> FrameWork {
    let before = renderer.frame_work();
    renderer.draw(scene, target).unwrap();
    renderer.frame_work().since(before)
}

fn busy(scene: &mut Scene) -> Vec<NodeId> {
    (0..4000)
        .map(|index| {
            let rect = Rect::new(
                (index % 40) as f32 * 24.0,
                (index / 40) as f32 * 10.0,
                22.0,
                9.0,
            );
            let node = match index % 4 {
                0 => Node::text(
                    rect,
                    TextSpec::new(format!("{index}"), 8.0).layout(Some(rect.width())),
                ),
                _ => Node::rounded_rect(rect, 2.0),
            };
            scene.insert(node.z(index).fill(Color::WHITE))
        })
        .collect()
}

#[test]
fn one_changed_node_reemits_only_it() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(1024, 1024).unwrap();
    let mut scene = Scene::new();
    let ids = busy(&mut scene);
    let first = drawn(&mut renderer, &mut scene, &mut target);
    assert_eq!(first.emitted, 4000);

    let started = Instant::now();
    let mut work = FrameWork::default();
    for frame in 0..200 {
        let shade = if frame % 2 == 0 {
            Color::BLACK
        } else {
            Color::WHITE
        };
        scene.edit(ids[7]).fill(shade);
        work.add(drawn(&mut renderer, &mut scene, &mut target));
    }
    renderer.read_pixels(&mut target).unwrap();
    eprintln!("one node: {:?} a frame, {work:?}", started.elapsed() / 200);
    assert_eq!(work.emitted, 200, "only the edited node is made again");
    assert_eq!(work.restructured, 0);
    assert_eq!(work.uploaded_instances, 200, "and only it is sent");
    assert!(work.walked <= 200);

    let started = Instant::now();
    let before = renderer.frame_work();
    for frame in 0..50 {
        for (z, id) in ids.iter().enumerate() {
            scene.edit(*id).z(z as i32 + frame + 1);
        }
        renderer.draw(&mut scene, &mut target).unwrap();
    }
    renderer.read_pixels(&mut target).unwrap();
    let shifted = renderer.frame_work().since(before);
    eprintln!(
        "shifted z: {:?} a frame, {shifted:?}",
        started.elapsed() / 50
    );
    assert_eq!(
        shifted.frames, 0,
        "an order that keeps its sequence draws nothing"
    );
    assert!(!scene.has_damage());
}

fn two_squares(scene: &mut Scene) -> (NodeId, NodeId) {
    scene.set_background(Color::BLACK);
    let red = scene.insert(
        Node::rect(Rect::new(0.0, 0.0, 40.0, 40.0))
            .z(1)
            .fill(Color::rgb(1.0, 0.0, 0.0)),
    );
    let blue = scene.insert(
        Node::rect(Rect::new(20.0, 20.0, 40.0, 40.0))
            .z(2)
            .fill(Color::rgb(0.0, 0.0, 1.0)),
    );
    (red, blue)
}

#[test]
fn alternating_scenes_upload_only_changes() {
    let mut renderer = renderer();
    let mut left = renderer.create_target(SIDE, SIDE).unwrap();
    let mut right = renderer.create_target(SIDE, SIDE).unwrap();
    let mut a = Scene::new();
    let mut b = Scene::new();
    let (a_red, _) = two_squares(&mut a);
    let (b_red, _) = two_squares(&mut b);
    for _ in 0..3 {
        drawn(&mut renderer, &mut a, &mut left);
        drawn(&mut renderer, &mut b, &mut right);
    }

    for frame in 0..6 {
        let shade = Color::rgb(0.0, (frame % 2) as f32, 0.0);
        a.edit(a_red).fill(shade);
        let on_a = drawn(&mut renderer, &mut a, &mut left);
        b.edit(b_red).fill(shade);
        let on_b = drawn(&mut renderer, &mut b, &mut right);
        assert_eq!(
            on_a.uploaded_instances, 1,
            "the other scene cost this one nothing"
        );
        assert_eq!(on_b.uploaded_instances, 1);
    }
    assert_eq!(pixel(&mut renderer, &mut left, 5, 5), [0, 255, 0, 255]);
    assert_eq!(pixel(&mut renderer, &mut right, 5, 5), [0, 255, 0, 255]);
}

#[test]
fn scene_text_layout_stays_per_scene() {
    let mut renderer = renderer();
    let mut thin = renderer.create_target(SIDE, SIDE).unwrap();
    let mut heavy = renderer.create_target(SIDE, SIDE).unwrap();
    let written = |text: &str| {
        let mut scene = Scene::new();
        scene.set_background(Color::BLACK);
        scene.insert(
            Node::text(
                Rect::new(0.0, 0.0, 64.0, 64.0),
                TextSpec::new(text, 20.0).layout(Some(64.0)),
            )
            .fill(Color::WHITE),
        );
        scene
    };
    let mut is = written("iiii");
    let mut ws = written("WWWW");

    renderer.draw(&mut is, &mut thin).unwrap();
    renderer.draw(&mut ws, &mut heavy).unwrap();
    let thin = lit(&renderer.read_pixels(&mut thin).unwrap());
    let heavy = lit(&renderer.read_pixels(&mut heavy).unwrap());
    assert!(
        heavy > thin * 2,
        "the second scene drew the first one's text: {thin} lit against {heavy}"
    );
}

#[test]
fn outer_group_clip_follows_outer_group() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let viewport = Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32);
    let page = scene.create_group();
    let scrolled = scene.create_group();
    scene.set_group(page, None, Point::ZERO, viewport);
    scene.set_group(scrolled, Some(page), Point::ZERO, viewport);
    let text = TextSpec::new("clipped", 12.0).layout(Some(40.0));
    scene.insert(
        Node::rect(Rect::new(0.0, 0.0, 40.0, 64.0))
            .clip(Rect::new(0.0, 0.0, 20.0, 64.0))
            .clip_in(page)
            .group(scrolled)
            .fill(Color::WHITE),
    );
    scene.insert(
        Node::text(Rect::new(0.0, 0.0, 40.0, 20.0), text)
            .clip(Rect::new(0.0, 0.0, 20.0, 64.0))
            .clip_in(page)
            .group(scrolled)
            .fill(Color::WHITE),
    );
    drawn(&mut renderer, &mut scene, &mut target);
    assert_eq!(pixel(&mut renderer, &mut target, 30, 30), [0, 0, 0, 255]);

    let mut work = FrameWork::default();
    for step in 1..=4 {
        scene.set_group(page, None, Point::new(step as f32 * 5.0, 0.0), viewport);
        work.add(drawn(&mut renderer, &mut scene, &mut target));
    }
    assert_eq!(
        pixel(&mut renderer, &mut target, 30, 30),
        [255, 255, 255, 255],
        "the clip went along with the page"
    );
    assert_eq!(pixel(&mut renderer, &mut target, 45, 30), [0, 0, 0, 255]);
    assert_eq!(work.emitted, 0, "turning the page makes nothing again");
    assert_eq!(work.uploaded_instances, 0);

    scene.set_group(scrolled, Some(page), Point::new(-10.0, 0.0), viewport);
    let scrolling = drawn(&mut renderer, &mut scene, &mut target);
    assert_eq!(
        pixel(&mut renderer, &mut target, 25, 30),
        [255, 255, 255, 255]
    );
    assert_eq!(
        pixel(&mut renderer, &mut target, 45, 30),
        [0, 0, 0, 255],
        "the clip stays where the page holds it"
    );
    assert_eq!(scrolling.emitted, 0);
}

#[test]
fn instance_buffer_growth_uploads_additions() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let spot = |index: usize| {
        Rect::new(
            (index % 32) as f32 * 2.0,
            (index / 32) as f32 * 2.0,
            2.0,
            2.0,
        )
    };
    for index in 0..8 {
        scene.insert(Node::rect(spot(index)).z(index as i32).fill(Color::WHITE));
    }
    drawn(&mut renderer, &mut scene, &mut target);

    let mut work = FrameWork::default();
    for index in 8..1000 {
        scene.insert(Node::rect(spot(index)).z(index as i32).fill(Color::WHITE));
        work.add(drawn(&mut renderer, &mut scene, &mut target));
    }
    assert_eq!(
        work.uploaded_instances, 992,
        "a larger buffer is filled on the GPU"
    );
    let lit = lit(&renderer.read_pixels(&mut target).unwrap());
    assert_eq!(lit, 1000 * 4, "everything added is drawn");
}

#[test]
fn inserted_nodes_avoid_relayout() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let red = Color::rgb(1.0, 0.0, 0.0);
    let blue = Color::rgb(0.0, 0.0, 1.0);
    let square =
        |at: f32, z: i32, fill: Color| Node::rect(Rect::new(at, at, 24.0, 24.0)).z(z).fill(fill);
    scene.insert(square(0.0, 10, red));
    scene.insert(square(20.0, 30, blue));
    let label = scene.insert(
        Node::text(
            Rect::new(36.0, 36.0, 28.0, 28.0),
            TextSpec::new("i", 20.0).layout(Some(28.0)),
        )
        .z(20)
        .fill(red),
    );
    scene.insert(
        Node::rect(Rect::new(36.0, 36.0, 28.0, 28.0))
            .z(40)
            .fill(blue),
    );
    drawn(&mut renderer, &mut scene, &mut target);

    scene.insert(square(10.0, 20, Color::rgb(0.0, 1.0, 0.0)));
    scene.insert(square(0.0, 5, Color::WHITE));
    scene
        .edit(label)
        .text(TextSpec::new("iiiiiiii", 20.0).layout(Some(28.0)));
    let work = drawn(&mut renderer, &mut scene, &mut target);

    assert_eq!(work.restructured, 0, "the arrivals were slotted in");
    assert_eq!(pixel(&mut renderer, &mut target, 5, 5), [255, 0, 0, 255]);
    assert_eq!(pixel(&mut renderer, &mut target, 15, 15), [0, 255, 0, 255]);
    assert_eq!(pixel(&mut renderer, &mut target, 25, 25), [0, 0, 255, 255]);
    let pixels = renderer.read_pixels(&mut target).unwrap();
    let covered = (36..64)
        .flat_map(|y| (36..64).map(move |x| (y * SIDE + x) as usize * 4))
        .filter(|at| pixels[*at] > 0)
        .count();
    assert_eq!(covered, 0, "the regrown text still paints under the cover");
}

#[test]
fn moved_text_shifts_without_reemit() {
    let mut renderer = renderer();
    let mut moved = renderer.create_target(SIDE, SIDE).unwrap();
    let mut fresh = renderer.create_target(SIDE, SIDE).unwrap();
    let words = || TextSpec::new("moved\nwords", 14.0).layout(Some(60.0));
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let label =
        scene.insert(Node::text(Rect::new(2.0, 1.25, 60.0, 30.0), words()).fill(Color::WHITE));
    drawn(&mut renderer, &mut scene, &mut moved);

    let there = Rect::new(3.0, 20.75, 60.0, 30.0);
    scene.edit(label).rect(there);
    let work = drawn(&mut renderer, &mut scene, &mut moved);
    assert_eq!(work.emitted, 0, "the glyphs were shifted, not made again");
    assert_eq!(work.uploaded_instances, 0, "only the node's move was sent");

    let mut placed = Scene::new();
    placed.set_background(Color::BLACK);
    placed.insert(Node::text(there, words()).fill(Color::WHITE));
    drawn(&mut renderer, &mut placed, &mut fresh);
    let shifted = renderer.read_pixels(&mut moved).unwrap();
    let fresh_pixels = renderer.read_pixels(&mut fresh).unwrap();
    assert!(lit(&fresh_pixels) > 0);
    assert!(
        shifted == fresh_pixels,
        "shifted text differs from text drawn in place"
    );
}

#[test]
fn subpixel_nudged_text_shifts_without_reemit() {
    let mut renderer = renderer();
    let mut moved = renderer.create_target(SIDE, SIDE).unwrap();
    let mut fresh = renderer.create_target(SIDE, SIDE).unwrap();
    let words = || TextSpec::new("i", 20.0).layout(Some(30.0));
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let label =
        scene.insert(Node::text(Rect::new(8.0, 8.0, 30.0, 30.0), words()).fill(Color::WHITE));
    drawn(&mut renderer, &mut scene, &mut moved);

    let there = Rect::new(9.05, 12.5, 30.0, 30.0);
    scene.edit(label).rect(there);
    let work = drawn(&mut renderer, &mut scene, &mut moved);
    assert_eq!(work.emitted, 0, "the bin held, so the glyph was shifted");

    let mut placed = Scene::new();
    placed.set_background(Color::BLACK);
    placed.insert(Node::text(there, words()).fill(Color::WHITE));
    drawn(&mut renderer, &mut placed, &mut fresh);
    let shifted = renderer.read_pixels(&mut moved).unwrap();
    let fresh_pixels = renderer.read_pixels(&mut fresh).unwrap();
    assert!(lit(&fresh_pixels) > 0);
    assert!(
        shifted == fresh_pixels,
        "shifted text differs from text drawn in place"
    );
}

#[test]
fn fading_layer_edit_avoids_relayout() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut fresh_target = renderer.create_target(SIDE, SIDE).unwrap();
    let square = |x: f32, y: f32, fill: Color| Node::rect(Rect::new(x, y, 16.0, 16.0)).fill(fill);
    let lay = |scene: &mut Scene, settled: bool| {
        scene.set_background(Color::BLACK);
        scene.insert(Node::rect(Rect::new(0.0, 0.0, 64.0, 32.0)).fill(Color::rgb(0.1, 0.4, 0.9)));
        let layer = scene.create_layer();
        scene.set_layer(layer, None, 0.5);
        let moving = scene.insert(
            square(
                match settled {
                    true => 40.0,
                    false => 4.0,
                },
                4.0,
                Color::WHITE,
            )
            .layer(layer),
        );
        let tinted = scene.insert(
            square(
                20.0,
                20.0,
                match settled {
                    true => Color::rgb(1.0, 0.0, 0.0),
                    false => Color::rgb(0.0, 1.0, 0.0),
                },
            )
            .layer(layer)
            .z(1),
        );
        (layer, moving, tinted)
    };
    let mut scene = Scene::new();
    let (layer, moving, tinted) = lay(&mut scene, false);
    drawn(&mut renderer, &mut scene, &mut target);

    scene.edit(moving).rect(Rect::new(40.0, 4.0, 16.0, 16.0));
    scene.edit(tinted).fill(Color::rgb(1.0, 0.0, 0.0));
    scene.insert(
        square(8.0, 44.0, Color::rgb(1.0, 1.0, 0.0))
            .layer(layer)
            .z(2),
    );
    let work = drawn(&mut renderer, &mut scene, &mut target);
    let got = renderer.read_pixels(&mut target).unwrap();

    let mut fresh = Scene::immediate();
    let (layer, _, _) = lay(&mut fresh, true);
    fresh.insert(
        square(8.0, 44.0, Color::rgb(1.0, 1.0, 0.0))
            .layer(layer)
            .z(2),
    );
    renderer.draw(&mut fresh, &mut fresh_target).unwrap();
    let want = renderer.read_pixels(&mut fresh_target).unwrap();

    assert_eq!(work.restructured, 0, "the layered edits were made in place");
    let off = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max();
    assert!(
        off.is_some_and(|off| off <= 1),
        "a layer edited in place differs from a fresh one by {off:?}"
    );
}

#[test]
fn small_group_move_damages_members() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let whole = Rect::new(0.0, 0.0, SIDE as f32, SIDE as f32);
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(Node::rect(whole).fill(Color::rgb(0.2, 0.2, 0.2)));
    drawn(&mut renderer, &mut scene, &mut target);

    let group = scene.create_group();
    scene.insert(
        Node::rect(Rect::new(4.0, 4.0, 8.0, 8.0))
            .fill(Color::WHITE)
            .group(group),
    );
    scene.set_group(group, None, Point::new(10.0, 0.0), whole);
    let first = renderer.draw(&mut scene, &mut target).unwrap();
    scene.set_group(group, None, Point::new(20.0, 6.0), whole);
    let moved = renderer.draw(&mut scene, &mut target).unwrap();

    let reach = Rect::new(4.0, 4.0, 28.0, 14.0);
    for (step, out) in [("placed", first), ("moved", moved)] {
        assert!(
            out.damage.iter().all(|rect| reach.contains_rect(rect)),
            "{step}: a group's move repaints only its members: {:?}",
            out.damage
        );
    }
}

#[test]
fn non_text_rect_move_uses_moves_table_with_zero_instance_uploads() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(SIDE, SIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let card = scene.insert(
        Node::rounded_rect(Rect::new(8.0, 8.0, 24.0, 24.0), 4.0).fill(Color::rgb(0.2, 0.8, 0.4)),
    );
    drawn(&mut renderer, &mut scene, &mut target);

    scene.edit(card).rect(Rect::new(32.0, 20.0, 24.0, 24.0));
    let moved = drawn(&mut renderer, &mut scene, &mut target);
    assert_eq!(
        moved.uploaded_instances, 0,
        "moving a same-size shape updates only the moves table"
    );
    assert_eq!(pixel(&mut renderer, &mut target, 16, 16), [0, 0, 0, 255]);
    assert_eq!(
        pixel(&mut renderer, &mut target, 44, 32),
        [51, 204, 102, 255]
    );
}
