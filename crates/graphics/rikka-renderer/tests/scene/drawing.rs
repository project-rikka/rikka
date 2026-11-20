use rikka_gpu::Pixels;
use rikka_renderer::{
    BackdropCache, Blur, ImageSource, Node, NodeId, PostEffect, Scene, Stroke, TRIM_AFTER,
    Transform,
};
use rikka_test_renderer::TestRenderer;
use rikka_text::{Inks, TextPaint, TextSpan, TextSpec};
use rikka_types::{Color, CornerRadii, Lens, Point, Rect, RoundedRect, Size};

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-test")
        .expect("a Vulkan 1.3 device is required for these tests")
}

type Pixel = (u8, u8, u8, u8);

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> Pixel {
    let i = (y * width + x) * 4;
    (pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3])
}

fn assert_pane_matches(name: &str, got: &[u8], want: &[u8], width: usize, pane: Rect) {
    for y in (pane.min_y() as usize)..(pane.max_y() as usize) {
        for x in (pane.min_x() as usize)..(pane.max_x() as usize) {
            let (got, want) = (pixel(got, width, x, y), pixel(want, width, x, y));
            let off = [
                got.0.abs_diff(want.0),
                got.1.abs_diff(want.1),
                got.2.abs_diff(want.2),
            ];
            assert!(
                off.iter().all(|channel| *channel <= 2),
                "{name}: at ({x}, {y}) came back {got:?} where a whole blur gives {want:?}"
            );
        }
    }
}

fn damage_bounds(damage: &[Rect]) -> Rect {
    damage
        .iter()
        .copied()
        .reduce(|all, rect| all.union(&rect))
        .expect("the frame painted something")
}

#[test]
fn solid_shapes_damage_incrementally() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    let rect = scene
        .insert(Node::rect(Rect::new(64.0, 64.0, 128.0, 128.0)).fill(Color::rgb(0.0, 0.0, 1.0)));

    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(out.rendered());
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 256, 10, 10), (255, 0, 0, 255), "background");
    assert_eq!(pixel(&px, 256, 128, 128), (0, 0, 255, 255), "rect fill");

    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(!out.rendered());

    scene.edit(rect).fill(Color::rgb(0.0, 1.0, 0.0));
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(out.rendered());
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 256, 128, 128), (0, 255, 0, 255));
    assert_eq!(pixel(&px, 256, 10, 10), (255, 0, 0, 255));
}

#[test]
fn circle_sdf_coverage() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    scene.insert(
        Node::circle(Point::new(128.0, 128.0), 64.0)
            .fill(Color::rgb(0.0, 0.0, 1.0))
            .stroke(Stroke::new(4.0, Color::WHITE)),
    );

    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 256, 128, 128), (0, 0, 255, 255), "circle center");
    assert_eq!(pixel(&px, 256, 70, 70), (255, 0, 0, 255), "bbox corner");
    assert_eq!(pixel(&px, 256, 128, 66), (255, 255, 255, 255), "stroke");
}

#[test]
fn clip_masks_node_to_rounded_corners() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    let window = Rect::new(64.0, 64.0, 128.0, 128.0);
    let disc = scene.insert(
        Node::circle(Point::new(128.0, 128.0), 200.0)
            .fill(Color::rgb(0.0, 0.0, 1.0))
            .clip(RoundedRect::new(window, 32.0)),
    );

    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 256, 128, 128), (0, 0, 255, 255), "middle");
    assert_eq!(pixel(&px, 256, 128, 70), (0, 0, 255, 255), "top edge");
    assert_eq!(pixel(&px, 256, 70, 70), (255, 0, 0, 255), "rounded corner");
    assert_eq!(pixel(&px, 256, 40, 128), (255, 0, 0, 255), "past the clip");

    scene.edit(disc).clip(RoundedRect::rect(window));
    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 256, 70, 70), (0, 0, 255, 255), "square corner");
    assert_eq!(pixel(&px, 256, 40, 128), (255, 0, 0, 255), "past the clip");
}

#[test]
fn text_renders_glyphs() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(512, 128).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(
        Node::text(
            Rect::new(8.0, 8.0, 496.0, 112.0),
            TextSpec::new("Rikka renderer 日本語テキスト", 24.0).layout(Some(496.0)),
        )
        .fill(Color::WHITE),
    );

    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    let lit = px
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 32 || p[1] > 32 || p[2] > 32)
        .count();
    assert!(lit > 200, "expected glyph coverage, found {lit} lit pixels");
}

#[test]
fn color_emoji_ignore_fill() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(128, 64).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(
        Node::text(
            Rect::new(8.0, 4.0, 112.0, 56.0),
            TextSpec::new("\u{1F600}\u{1F49A}", 40.0).layout(Some(112.0)),
        )
        .fill(Color::WHITE),
    );

    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    let pixels = px.as_chunks::<4>().0;
    let yellow = pixels
        .iter()
        .filter(|p| p[0] > 160 && p[1] > 120 && p[2] < 90)
        .count();
    let green = pixels
        .iter()
        .filter(|p| p[1] > 100 && p[1] > p[0].saturating_add(30) && p[1] > p[2].saturating_add(30))
        .count();
    assert!(yellow > 100, "the face drew {yellow} yellow pixels");
    assert!(green > 100, "the heart drew {green} green pixels");
}

#[test]
fn atlas_page_growth_preserves_glyphs() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(512, 128).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);

    let many: String = (0x4e00..0x4e00 + 900_u32)
        .filter_map(char::from_u32)
        .collect();
    let crowd = scene.insert(
        Node::text(
            Rect::new(0.0, 0.0, 512.0, 128.0),
            TextSpec::new(many, 28.0)
                .line_height(30.0)
                .layout(Some(512.0)),
        )
        .fill(Color::WHITE),
    );
    renderer.draw(&mut scene, &mut target).unwrap();

    scene.remove(crowd);
    scene.insert(
        Node::text(
            Rect::new(8.0, 8.0, 496.0, 112.0),
            TextSpec::new("Rikka renderer 日本語テキスト", 24.0).layout(Some(496.0)),
        )
        .fill(Color::WHITE),
    );
    renderer.draw(&mut scene, &mut target).unwrap();

    let px = renderer.read_pixels(&mut target).unwrap();
    let lit = px
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 32 || p[1] > 32 || p[2] > 32)
        .count();
    assert!(lit > 200, "expected glyph coverage, found {lit} lit pixels");
}

#[test]
fn output_transform_rotates_scene() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(320, 200).unwrap();
    target.set_transform(Transform::Rotate90);

    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));
    scene.insert(Node::rect(Rect::new(0.0, 0.0, 40.0, 40.0)).fill(Color::rgb(0.0, 0.0, 1.0)));

    assert!(renderer.draw(&mut scene, &mut target).unwrap().rendered());
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 320, 20, 180), (0, 0, 255, 255), "bottom-left");
    assert_eq!(pixel(&px, 320, 20, 20), (255, 0, 0, 255), "top-left");
    assert_eq!(pixel(&px, 320, 300, 180), (255, 0, 0, 255), "bottom-right");

    target.set_transform(Transform::Normal);
    assert!(renderer.draw(&mut scene, &mut target).unwrap().rendered());
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 320, 20, 20), (0, 0, 255, 255), "top-left");
    assert_eq!(pixel(&px, 320, 300, 20), (255, 0, 0, 255), "top-right");
}

#[test]
fn scenes_sharing_renderer_stay_separate() {
    let mut renderer = renderer();
    let mut left = renderer.create_target(64, 64).unwrap();
    let mut right = renderer.create_target(64, 64).unwrap();

    let mut blue = Scene::new();
    blue.set_background(Color::rgb(1.0, 0.0, 0.0));
    blue.insert(Node::rect(Rect::new(0.0, 0.0, 64.0, 64.0)).fill(Color::rgb(0.0, 0.0, 1.0)));

    let mut green = Scene::new();
    green.set_background(Color::rgb(1.0, 0.0, 0.0));
    green.insert(Node::rect(Rect::new(0.0, 0.0, 64.0, 64.0)).fill(Color::rgb(0.0, 1.0, 0.0)));

    renderer.draw(&mut blue, &mut left).unwrap();
    renderer.draw(&mut green, &mut right).unwrap();

    let px = renderer.read_pixels(&mut left).unwrap();
    assert_eq!(pixel(&px, 64, 32, 32), (0, 0, 255, 255));
    let px = renderer.read_pixels(&mut right).unwrap();
    assert_eq!(pixel(&px, 64, 32, 32), (0, 255, 0, 255));
}

fn ramp(profile: impl Iterator<Item = u8>) -> usize {
    profile.filter(|value| (32..=223).contains(value)).count()
}

#[test]
fn blur_reach_is_isotropic() {
    const PANE: Rect = Rect::new(32.0, 32.0, 192.0, 192.0);

    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();

    let glassed = |half: Rect| {
        let mut scene = Scene::new();
        scene.set_background(Color::BLACK);
        scene.insert(Node::rect(half).fill(Color::WHITE));
        scene.insert(
            Node::rounded_rect(PANE, 0.0)
                .fill(Color::TRANSPARENT)
                .backdrop(Blur::new(12.0)),
        );
        scene
    };

    let mut across = glassed(Rect::new(0.0, 0.0, 128.0, 256.0));
    renderer.draw(&mut across, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    let horizontal = ramp((96..160).map(|x| pixel(&px, 256, x, 128).0));

    let mut down = glassed(Rect::new(0.0, 0.0, 256.0, 128.0));
    renderer.draw(&mut down, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    let vertical = ramp((96..160).map(|y| pixel(&px, 256, 128, y).0));

    assert!(
        (24..=32).contains(&horizontal) && (24..=32).contains(&vertical),
        "a blur of twelve pixels spread {horizontal} across and {vertical} down"
    );
    assert!(
        horizontal.abs_diff(vertical) <= 3,
        "the blur reached {horizontal} pixels across and {vertical} down"
    );
}

#[test]
fn pane_past_target_edge_keeps_backdrop_in_place() {
    const NEAR: Color = Color::rgb(1.0, 0.0, 0.0);
    const FAR: Color = Color::rgb(0.0, 0.0, 1.0);

    let cases = [
        (
            "left",
            Rect::new(0.0, 0.0, 128.0, 256.0),
            Rect::new(-512.0, 32.0, 768.0, 192.0),
            [(64, 128), (192, 128)],
        ),
        (
            "top",
            Rect::new(0.0, 0.0, 256.0, 128.0),
            Rect::new(32.0, -512.0, 192.0, 768.0),
            [(128, 64), (128, 192)],
        ),
    ];

    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    for (edge, near, pane, [inside, outside]) in cases {
        let mut scene = Scene::new();
        scene.set_background(FAR);
        scene.insert(Node::rect(near).fill(NEAR));
        scene.insert(
            Node::rounded_rect(pane, 0.0)
                .fill(Color::TRANSPARENT)
                .backdrop(Blur::new(3.0)),
        );
        renderer.draw(&mut scene, &mut target).unwrap();
        let px = renderer.read_pixels(&mut target).unwrap();

        let (near_at, far_at) = (
            pixel(&px, 256, inside.0, inside.1),
            pixel(&px, 256, outside.0, outside.1),
        );
        assert!(
            near_at.0 > 200 && near_at.2 < 55 && far_at.2 > 200 && far_at.0 < 55,
            "{edge}: glass past the edge shifted its backdrop: {near_at:?} over red, {far_at:?} over blue"
        );
    }
}

#[test]
fn partial_damage_never_clears_target() {
    const RED: (u8, u8, u8, u8) = (255, 0, 0, 255);

    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();

    let mut whole = Scene::new();
    whole.set_background(Color::rgb(1.0, 0.0, 0.0));
    renderer.draw(&mut whole, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 256, 200, 200), RED, "painted whole");

    let mut piece = Scene::immediate();
    let corner = Rect::new(0.0, 0.0, 32.0, 32.0);
    piece.insert(
        Node::rounded_rect(corner, 0.0)
            .fill(Color::WHITE.with_alpha(0.5))
            .clip(corner)
            .backdrop(Blur::new(6.0)),
    );
    let out = renderer.draw(&mut piece, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();

    assert_eq!(
        pixel(&px, 256, 200, 200),
        RED,
        "what nothing damaged is still there"
    );
    assert!(
        !out.needs_full_redraw,
        "glass reads the target it lands on, so no full redraw is needed"
    );
}

#[test]
fn strip_reblur_matches_whole_blur() {
    const WHOLE: Rect = Rect::new(0.0, 0.0, 256.0, 256.0);
    const PANE: Rect = Rect::new(32.0, 32.0, 192.0, 192.0);
    const PATCH: Rect = Rect::new(96.0, 0.0, 64.0, 256.0);
    const GROUND: Color = Color::rgb(0.9, 0.1, 0.1);
    const PATCHED: Color = Color::rgb(0.1, 0.2, 0.9);

    fn desktop(scene: &mut Scene, damage: Rect, patched: bool, cache: BackdropCache) {
        scene.restart();
        scene.insert(Node::rect(damage).fill(GROUND).clip(damage));
        if patched && let Some(clip) = PATCH.intersection(&damage) {
            scene.insert(Node::rect(PATCH).fill(PATCHED).clip(clip));
        }
        scene.insert(
            Node::rounded_rect(PANE, 0.0)
                .fill(Color::TRANSPARENT)
                .clip(damage)
                .backdrop(Blur::with_radius(24.0))
                .backdrop_cache(cache),
        );
    }

    let reach = Blur::with_radius(24.0).spread(WHOLE.size);
    let read = PATCH
        .inflate(reach)
        .round_out()
        .intersection(&WHOLE)
        .unwrap();

    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    let mut scene = Scene::immediate();

    desktop(&mut scene, WHOLE, false, BackdropCache::stale(1, [WHOLE]));
    renderer.draw(&mut scene, &mut target).unwrap();
    desktop(&mut scene, read, true, BackdropCache::stale(1, [read]));
    renderer.draw(&mut scene, &mut target).unwrap();
    let strips = renderer.read_pixels(&mut target).unwrap();

    desktop(&mut scene, WHOLE, true, BackdropCache::stale(2, [WHOLE]));
    renderer.draw(&mut scene, &mut target).unwrap();
    let filtered_whole = renderer.read_pixels(&mut target).unwrap();

    assert_pane_matches("the strips", &strips, &filtered_whole, 256, PANE);
}
#[test]
fn stale_strip_reblurs_locally() {
    const WIDTH: usize = 512;
    const WHOLE: Rect = Rect::new(0.0, 0.0, 512.0, 128.0);
    const PANE: Rect = Rect::new(16.0, 16.0, 480.0, 96.0);
    const PATCH: Rect = Rect::new(32.0, 0.0, 24.0, 128.0);
    const GROUND: Color = Color::rgb(0.9, 0.1, 0.1);
    const PATCHED: Color = Color::rgb(0.1, 0.2, 0.9);

    fn desktop(patch: Color, key: u64) -> (Scene, NodeId, NodeId) {
        let mut scene = Scene::new();
        scene.insert(Node::rect(WHOLE).fill(GROUND));
        let patched = scene.insert(Node::rect(PATCH).fill(patch));
        let pane = scene.insert(
            Node::rounded_rect(PANE, 0.0)
                .fill(Color::TRANSPARENT)
                .backdrop(Blur::with_radius(24.0))
                .backdrop_cache(BackdropCache::current(key)),
        );
        (scene, patched, pane)
    }

    let reach = Blur::with_radius(24.0).spread(PANE.size);
    let stale = PATCH
        .inflate(reach)
        .round_out()
        .intersection(&WHOLE)
        .unwrap();

    let mut renderer = renderer();
    renderer.retain_backdrops(&[1, 2]);
    let mut target = renderer.create_target(512, 128).unwrap();
    let (mut scene, patched, pane) = desktop(GROUND, 1);
    renderer.draw(&mut scene, &mut target).unwrap();
    scene.edit(patched).fill(PATCHED);
    scene
        .edit(pane)
        .backdrop_cache(BackdropCache::stale(1, [stale]));
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    let repaired = renderer.read_pixels(&mut target).unwrap();

    let furthest = out.damage.iter().map(Rect::max_x).fold(0.0, f32::max);
    assert!(
        furthest < WHOLE.max_x() - 64.0,
        "a stale strip repaints near itself, not the whole pane: {:?}",
        out.damage
    );

    let (mut fresh, _, _) = desktop(PATCHED, 2);
    renderer.draw(&mut fresh, &mut target).unwrap();
    let whole = renderer.read_pixels(&mut target).unwrap();

    assert_pane_matches("the repaired pane", &repaired, &whole, WIDTH, PANE);
}

#[test]
fn stale_lens_core_reblurs_locally() {
    const WIDTH: usize = 768;
    const WHOLE: Rect = Rect::new(0.0, 0.0, 768.0, 512.0);
    const PANE: Rect = Rect::new(16.0, 16.0, 736.0, 480.0);
    const GROUND: Color = Color::rgb(0.9, 0.1, 0.1);
    const STRIPE: Color = Color::rgb(0.1, 0.7, 0.3);
    const PATCHED: Color = Color::rgb(0.1, 0.2, 0.9);

    fn desktop(patch: Rect, colour: Color, key: u64) -> (Scene, NodeId, NodeId) {
        let mut scene = Scene::new();
        scene.insert(Node::rect(WHOLE).fill(GROUND));
        for left in (0..768).step_by(64) {
            scene.insert(Node::rect(Rect::new(left as f32, 0.0, 24.0, 512.0)).fill(STRIPE));
        }
        let patched = scene.insert(Node::rect(patch).fill(colour));
        let pane = scene.insert(
            Node::rounded_rect(PANE, 16.0)
                .fill(Color::TRANSPARENT)
                .backdrop(Blur::with_radius(24.0))
                .lens(Lens::new())
                .backdrop_cache(BackdropCache::current(key)),
        );
        (scene, patched, pane)
    }

    let softened = Blur::with_radius(24.0).spread(PANE.size);
    let profile = Lens::new().profile_for_size(PANE.size);
    let shows = softened + profile.max_displacement();
    let cases = [
        (
            "core",
            Rect::new(372.0, 244.0, 24.0, 24.0),
            softened + profile.bulge_displacement,
            true,
        ),
        ("rim", Rect::new(24.0, 200.0, 24.0, 24.0), shows, false),
        ("beside", Rect::new(0.0, 120.0, 12.0, 24.0), shows, false),
    ];

    let mut renderer = renderer();
    let mut key = 0;
    for (name, patch, reach, local) in cases {
        key += 2;
        renderer.retain_backdrops(&[key, key + 1]);
        let mut target = renderer.create_target(768, 512).unwrap();
        let (mut scene, patched, pane) = desktop(patch, GROUND, key);
        renderer.draw(&mut scene, &mut target).unwrap();
        let stale = patch
            .inflate(reach)
            .round_out()
            .intersection(&PANE.inflate(8.0))
            .unwrap();
        scene.edit(patched).fill(PATCHED);
        scene
            .edit(pane)
            .backdrop_cache(BackdropCache::stale(key, [stale]));
        let out = renderer.draw(&mut scene, &mut target).unwrap();
        let repaired = renderer.read_pixels(&mut target).unwrap();

        if local {
            let reached = damage_bounds(&out.damage);
            assert!(
                reached.width() < PANE.width() * 0.6 && reached.height() < PANE.height() * 0.8,
                "{name}: a change deep inside the glass repaints near itself, not the whole pane: {reached:?}"
            );
        }

        let (mut fresh, _, _) = desktop(patch, PATCHED, key + 1);
        renderer.draw(&mut fresh, &mut target).unwrap();
        let whole = renderer.read_pixels(&mut target).unwrap();

        assert_pane_matches(name, &repaired, &whole, WIDTH, PANE);
    }
}

#[test]
fn lens_corner_change_repaints_corners() {
    const WIDTH: usize = 768;
    const WHOLE: Rect = Rect::new(0.0, 0.0, 768.0, 512.0);
    const PANE: Rect = Rect::new(16.0, 16.0, 736.0, 480.0);

    fn desktop(radii: CornerRadii, cache: Option<BackdropCache>) -> (Scene, NodeId) {
        let mut scene = Scene::new();
        scene.insert(Node::rect(WHOLE).fill(Color::rgb(0.9, 0.1, 0.1)));
        for left in (0..768).step_by(64) {
            scene.insert(
                Node::rect(Rect::new(left as f32, 0.0, 24.0, 512.0))
                    .fill(Color::rgb(0.1, 0.7, 0.3)),
            );
        }
        let mut pane = Node::rounded_rect(PANE, radii)
            .fill(Color::TRANSPARENT)
            .backdrop(Blur::with_radius(24.0))
            .lens(Lens::new());
        if let Some(cache) = cache {
            pane = pane.backdrop_cache(cache);
        }
        let pane = scene.insert(pane);
        (scene, pane)
    }

    let rounded = CornerRadii::uniform(16.0);
    let cases = [
        ("square, kept by its owner", CornerRadii::ZERO, true),
        ("square, kept by the scene", CornerRadii::ZERO, false),
        (
            "one corner, kept by its owner",
            CornerRadii::new(16.0, 16.0, 4.0, 16.0),
            true,
        ),
    ];

    let mut renderer = renderer();
    for (index, (name, radii, owned)) in cases.into_iter().enumerate() {
        let key = 100 + index as u64 * 2;
        renderer.retain_backdrops(&[key, key + 1]);
        let mut target = renderer.create_target(768, 512).unwrap();
        let (mut scene, pane) = desktop(rounded, owned.then(|| BackdropCache::current(key)));
        renderer.draw(&mut scene, &mut target).unwrap();
        scene.edit(pane).radii(radii);
        let out = renderer.draw(&mut scene, &mut target).unwrap();
        let reshaped = renderer.read_pixels(&mut target).unwrap();

        let middle = Rect::new(256.0, 176.0, 256.0, 160.0);
        assert!(
            !out.damage.iter().any(|rect| rect.intersects(&middle)),
            "{name}: rounding the corners anew repaints near them, not the whole pane: {:?}",
            out.damage
        );

        let (mut fresh, _) = desktop(radii, owned.then(|| BackdropCache::current(key + 1)));
        renderer.draw(&mut fresh, &mut target).unwrap();
        let whole = renderer.read_pixels(&mut target).unwrap();

        assert_pane_matches(name, &reshaped, &whole, WIDTH, PANE);
    }
}

#[test]
fn implicit_backdrop_cache_reblurs_locally() {
    const WIDTH: usize = 1024;
    const WHOLE: Rect = Rect::new(0.0, 0.0, 1024.0, 512.0);
    const PANE: Rect = Rect::new(288.0, 64.0, 480.0, 352.0);
    const GROUND: Color = Color::rgb(0.9, 0.1, 0.1);
    const STRIPE: Color = Color::rgb(0.1, 0.7, 0.3);
    const PATCHED: Color = Color::rgb(0.1, 0.2, 0.9);

    fn desktop(patch: Rect, colour: Color, lens: Option<Lens>) -> (Scene, NodeId) {
        let mut scene = Scene::new();
        scene.insert(Node::rect(WHOLE).fill(GROUND));
        for top in (0..512).step_by(48) {
            scene.insert(Node::rect(Rect::new(0.0, top as f32, 1024.0, 16.0)).fill(STRIPE));
        }
        let patched = scene.insert(Node::rect(patch).fill(colour));
        let mut pane = Node::rounded_rect(PANE, 12.0)
            .fill(Color::WHITE.with_alpha(0.2))
            .backdrop(Blur::with_radius(24.0));
        if let Some(lens) = lens {
            pane = pane.lens(lens);
        }
        scene.insert(pane);
        (scene, patched)
    }

    let centre = Rect::new(516.0, 228.0, 24.0, 24.0);
    let cases = [
        ("plain centre", None, centre, true),
        ("lensed centre", Some(Lens::new()), centre, true),
        (
            "lensed rim",
            Some(Lens::new()),
            Rect::new(296.0, 218.0, 24.0, 24.0),
            false,
        ),
        (
            "lensed corner",
            Some(Lens::new()),
            Rect::new(742.0, 378.0, 20.0, 20.0),
            false,
        ),
        (
            "lensed beside",
            Some(Lens::new()),
            Rect::new(512.0, 48.0, 24.0, 12.0),
            false,
        ),
        (
            "lensed afar",
            Some(Lens::new()),
            Rect::new(130.0, 230.0, 24.0, 24.0),
            false,
        ),
    ];

    let mut renderer = renderer();
    for (name, lens, patch, local) in cases {
        let mut target = renderer.create_target(1024, 512).unwrap();
        let (mut scene, patched) = desktop(patch, GROUND, lens);
        renderer.draw(&mut scene, &mut target).unwrap();
        scene.edit(patched).fill(PATCHED);
        let out = renderer.draw(&mut scene, &mut target).unwrap();
        let repaired = renderer.read_pixels(&mut target).unwrap();

        if local {
            let reached = damage_bounds(&out.damage);
            assert!(
                reached.width() < PANE.width() * 0.8 && reached.height() < PANE.height() * 0.9,
                "{name}: a change behind the glass repaints near itself, not the whole pane: {reached:?}"
            );
        }

        let (mut fresh, _) = desktop(patch, PATCHED, lens);
        renderer.draw(&mut fresh, &mut target).unwrap();
        let whole = renderer.read_pixels(&mut target).unwrap();

        assert_pane_matches(name, &repaired, &whole, WIDTH, PANE);
    }
}

#[test]
fn moved_pane_repaints_its_reach() {
    const WIDTH: usize = 640;
    const WHOLE: Rect = Rect::new(0.0, 0.0, 640.0, 384.0);
    const PANE: Rect = Rect::new(160.0, 96.0, 256.0, 160.0);
    const MOVED: Rect = Rect::new(184.0, 112.0, 256.0, 160.0);
    const OVER: Rect = Rect::new(448.0, 140.0, 40.0, 40.0);
    const ACROSS: Rect = Rect::new(200.0, 60.0, 120.0, 30.0);

    fn desktop(pane: Rect, key: u64) -> (Scene, NodeId) {
        let mut scene = Scene::new();
        scene.insert(Node::rect(WHOLE).fill(Color::rgb(0.9, 0.1, 0.1)));
        for left in (0..640).step_by(40) {
            scene.insert(
                Node::rect(Rect::new(left as f32, 0.0, 14.0, 384.0))
                    .fill(Color::rgb(0.1, 0.7, 0.3)),
            );
        }
        let glass = scene.insert(
            Node::rounded_rect(pane, 16.0)
                .fill(Color::WHITE.with_alpha(0.2))
                .backdrop(Blur::with_radius(24.0))
                .backdrop_cache(BackdropCache::current(key))
                .lens(Lens::new().scaled(0.5)),
        );
        scene.insert(Node::rect(OVER).fill(Color::rgb(0.95, 0.9, 0.2)));
        scene.insert(Node::rect(ACROSS).fill(Color::rgb(0.2, 0.2, 0.9).with_alpha(0.6)));
        (scene, glass)
    }

    let mut renderer = renderer();
    renderer.retain_backdrops(&[1, 2]);
    let mut target = renderer.create_target(640, 384).unwrap();
    let (mut scene, glass) = desktop(PANE, 1);
    renderer.draw(&mut scene, &mut target).unwrap();
    scene
        .edit(glass)
        .rect(MOVED)
        .backdrop_cache(BackdropCache::stale(1, [WHOLE]));
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    let moved = renderer.read_pixels(&mut target).unwrap();

    let allowed = PANE.union(&MOVED).union(&OVER).union(&ACROSS);
    assert!(
        out.damage.iter().all(|rect| allowed.contains_rect(rect)),
        "a moved pane repaints only itself and what lies over its reach: {:?}",
        out.damage
    );

    let mut fresh_target = renderer.create_target(640, 384).unwrap();
    let (mut fresh, _) = desktop(MOVED, 2);
    renderer.draw(&mut fresh, &mut fresh_target).unwrap();
    let whole = renderer.read_pixels(&mut fresh_target).unwrap();
    assert_pane_matches("the moved pane", &moved, &whole, WIDTH, WHOLE);
}

#[test]
fn readback_per_display_in_one_cycle() {
    let shades = [
        (Color::rgb(1.0, 0.0, 0.0), (255, 0, 0, 255)),
        (Color::rgb(0.0, 1.0, 0.0), (0, 255, 0, 255)),
        (Color::rgb(0.0, 0.0, 1.0), (0, 0, 255, 255)),
    ];

    let mut renderer = renderer();
    let mut displays: Vec<_> = shades
        .iter()
        .map(|_| {
            (
                renderer.create_target(64, 64).unwrap(),
                Scene::new(),
                renderer.create_readback(),
            )
        })
        .collect();

    for cycle in 0..shades.len() {
        for (display, (target, scene, readback)) in displays.iter_mut().enumerate() {
            scene.set_background(shades[(display + cycle) % shades.len()].0);
            renderer.draw(scene, &mut *target).unwrap();
            renderer.start_readback(target, readback).unwrap();
        }
        for (display, (_, _, readback)) in displays.iter_mut().enumerate() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !renderer.poll_readback(readback) {
                assert!(
                    std::time::Instant::now() < deadline,
                    "display {display} never finished its copy"
                );
            }
            assert_eq!(
                pixel(readback.pixels(), 64, 32, 32),
                shades[(display + cycle) % shades.len()].1,
                "display {display} on cycle {cycle}"
            );
        }
    }
}

fn quarters(renderer: &mut TestRenderer, size: u32) -> rikka_renderer::TextureId {
    const QUARTERS: [[u8; 4]; 4] = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ];

    let half = size / 2;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let quarter = usize::from(y >= half) * 2 + usize::from(x >= half);
            pixels.extend_from_slice(&QUARTERS[quarter]);
        }
    }
    renderer
        .textures()
        .create(Pixels::premultiplied(size, size, pixels))
        .unwrap()
}

type Quarters = [Pixel; 4];

const RED: Pixel = (255, 0, 0, 255);
const GREEN: Pixel = (0, 255, 0, 255);
const BLUE: Pixel = (0, 0, 255, 255);
const WHITE: Pixel = (255, 255, 255, 255);

#[test]
fn buffer_transforms_match_wl_output() {
    const CASES: [(Transform, Quarters); 8] = [
        (Transform::Normal, [RED, GREEN, BLUE, WHITE]),
        (Transform::Rotate90, [BLUE, RED, WHITE, GREEN]),
        (Transform::Rotate180, [WHITE, BLUE, GREEN, RED]),
        (Transform::Rotate270, [GREEN, WHITE, RED, BLUE]),
        (Transform::Flipped, [GREEN, RED, WHITE, BLUE]),
        (Transform::Flipped90, [RED, BLUE, GREEN, WHITE]),
        (Transform::Flipped180, [BLUE, WHITE, RED, GREEN]),
        (Transform::Flipped270, [WHITE, GREEN, BLUE, RED]),
    ];

    let mut renderer = renderer();
    let texture = quarters(&mut renderer, 128);
    let mut target = renderer.create_target(128, 128).unwrap();

    for (transform, expected) in CASES {
        let mut scene = Scene::new();
        scene.set_background(Color::BLACK);
        scene.insert(
            Node::image(Rect::new(0.0, 0.0, 128.0, 128.0), texture)
                .source(ImageSource::whole().transform(transform)),
        );
        renderer.draw(&mut scene, &mut target).unwrap();
        let px = renderer.read_pixels(&mut target).unwrap();
        for (quarter, want) in expected.into_iter().enumerate() {
            let (x, y) = (32 + (quarter & 1) * 64, 32 + (quarter >> 1) * 64);
            assert_eq!(pixel(&px, 128, x, y), want, "{transform:?} at ({x}, {y})");
        }
    }

    renderer.textures().destroy(texture);
}

fn turned_pixel(transform: Transform, x: usize, y: usize, w: usize, h: usize) -> (usize, usize) {
    let centre = Point::new(x as f32 + 0.5, y as f32 + 0.5);
    let turned = transform.map_point(centre, Size::new(w as f32, h as f32));
    (turned.x as usize, turned.y as usize)
}

fn glass_over_fade_scene() -> Scene {
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(Node::rect(Rect::new(0.0, 0.0, 48.0, 64.0)).fill(Color::rgb(0.9, 0.1, 0.1)));
    scene.insert(Node::rect(Rect::new(48.0, 0.0, 48.0, 32.0)).fill(Color::rgb(0.1, 0.8, 0.2)));
    scene.insert(Node::rect(Rect::new(48.0, 32.0, 48.0, 32.0)).fill(Color::rgb(0.1, 0.2, 0.9)));
    let layer = scene.create_layer();
    scene.set_layer(layer, None, 0.5);
    for left in [4.0, 20.0] {
        scene.insert(
            Node::rect(Rect::new(left, 36.0, 32.0, 20.0))
                .fill(Color::WHITE)
                .layer(layer),
        );
    }
    scene.insert(
        Node::rounded_rect(Rect::new(16.0, 8.0, 64.0, 44.0), 12.0)
            .fill(Color::WHITE.with_alpha(0.1))
            .backdrop(Blur::new(3.0))
            .lens(Lens::new().with_bevel(8.0).with_depth(6.0)),
    );
    scene
}

#[test]
fn glass_over_fade_on_rotated_target() {
    const W: usize = 96;
    const H: usize = 64;
    let mut renderer = renderer();
    let mut upright = renderer.create_target(W as u32, H as u32).unwrap();
    renderer
        .draw(&mut glass_over_fade_scene(), &mut upright)
        .unwrap();
    let want = renderer.read_pixels(&mut upright).unwrap();

    for transform in [
        Transform::Rotate90,
        Transform::Rotate180,
        Transform::Rotate270,
        Transform::Flipped,
        Transform::Flipped90,
        Transform::Flipped180,
        Transform::Flipped270,
    ] {
        let (tw, th) = match transform.swaps_axes() {
            true => (H as u32, W as u32),
            false => (W as u32, H as u32),
        };
        let mut target = renderer.create_target(tw, th).unwrap();
        target.set_transform(transform);
        renderer
            .draw(&mut glass_over_fade_scene(), &mut target)
            .unwrap();
        let got = renderer.read_pixels(&mut target).unwrap();
        for y in 0..H {
            for x in 0..W {
                let (tx, ty) = turned_pixel(transform, x, y, W, H);
                let (a, b) = (pixel(&got, tw as usize, tx, ty), pixel(&want, W, x, y));
                let off = [
                    a.0.abs_diff(b.0),
                    a.1.abs_diff(b.1),
                    a.2.abs_diff(b.2),
                    a.3.abs_diff(b.3),
                ];
                assert!(
                    off.iter().all(|&d| d <= 2),
                    "{transform:?} at ({x}, {y}) drew {a:?}, upright {b:?}"
                );
            }
        }
    }
}

#[test]
fn crowded_scene_partial_redraw_matches_full() {
    const WIDE: u32 = 256;
    const SIDE: i32 = 20;

    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDE, WIDE).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.1, 0.1, 0.1));

    let mut tiles = Vec::new();
    for y in 0..SIDE {
        for x in 0..SIDE {
            let at = Rect::new(x as f32 * 12.0 + 2.0, y as f32 * 12.0 + 2.0, 9.0, 9.0);
            let shade = (x + y) as f32 / (2.0 * SIDE as f32);
            tiles.push(scene.insert(Node::rounded_rect(at, 2.0).fill(Color::rgb(shade, 0.2, 0.8))));
        }
    }
    scene.insert(Node::text(
        Rect::new(8.0, 200.0, 240.0, 40.0),
        TextSpec::new("the quick brown fox jumps over the lazy dog", 16.0).layout(Some(240.0)),
    ));

    renderer.draw(&mut scene, &mut target).unwrap();
    let whole = renderer.read_pixels(&mut target).unwrap();

    let moved = tiles[SIDE as usize * 7 + 6];
    scene.edit(moved).fill(Color::rgb(1.0, 1.0, 0.0));
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    assert!(out.rendered(), "a change to one tile is a repair");
    let repaired = renderer.read_pixels(&mut target).unwrap();

    scene.damage_all();
    renderer.draw(&mut scene, &mut target).unwrap();
    let again = renderer.read_pixels(&mut target).unwrap();

    assert_ne!(whole, repaired, "the tile that changed is in the picture");
    assert_eq!(
        repaired, again,
        "a scene repaired in one place is what drawing all of it gives"
    );
}

#[test]
fn text_last_row_change_damages_row() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let words = "first row of words\nsecond row of words\nthird row of words";
    let layout = TextSpec::new(words, 16.0).layout(Some(240.0));
    let tail = words.rfind('\n').expect("three rows") + 1..words.len();
    let with_spans = |color: Color| TextPaint {
        inks: Inks::from_spans(words, &[TextSpan::new(tail.clone()).color(color)]),
        ..TextPaint::new(layout.clone())
    };
    let text = scene.insert(
        Node::text(Rect::new(8.0, 8.0, 240.0, 200.0), with_spans(Color::WHITE)).fill(Color::WHITE),
    );
    renderer.draw(&mut scene, &mut target).unwrap();

    scene.edit(text).text(with_spans(Color::rgb(1.0, 0.2, 0.2)));
    let out = renderer.draw(&mut scene, &mut target).unwrap();
    let first_row_bottom = 8.0 + layout.rows().next().expect("a first row").bottom();
    assert!(
        damage_bounds(&out.damage).min_y() >= first_row_bottom,
        "an ink change in the last row repainted {:?}, reaching the first row",
        damage_bounds(&out.damage)
    );
    let repaired = renderer.read_pixels(&mut target).unwrap();

    scene.damage_all();
    renderer.draw(&mut scene, &mut target).unwrap();
    let again = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(
        repaired, again,
        "text repainted near its changed row is what drawing all of it gives"
    );
}

#[test]
fn removed_frame_damages_its_area() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(1.0, 0.0, 0.0));

    let group = scene.create_group();
    let shown = Rect::new(0.0, 0.0, 256.0, 256.0);
    scene.set_group(group, None, Point::new(0.0, 96.0), shown);
    let frame_node = scene.insert(
        Node::rect(Rect::new(64.0, 0.0, 64.0, 64.0))
            .fill(Color::rgb(0.0, 0.0, 1.0))
            .clip(RoundedRect::rect(shown))
            .group(group),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 256, 96, 128), (0, 0, 255, 255), "where it stood");

    scene.remove_group(group);
    scene.remove(frame_node);
    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();

    assert_eq!(
        pixel(&px, 256, 96, 128),
        (255, 0, 0, 255),
        "what the frame showed was left on the target"
    );
}

fn overlapped(opacity: f32) -> Scene {
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.0));
    let layer = scene.create_layer();
    scene.set_layer(layer, None, opacity);
    for left in [32.0, 96.0] {
        scene.insert(
            Node::rect(Rect::new(left, 32.0, 96.0, 96.0))
                .fill(Color::WHITE)
                .layer(layer),
        );
    }
    scene
}

#[test]
fn layer_opacity_applies_once() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(256, 256).unwrap();
    let mut scene = overlapped(0.5);

    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();

    let alone = pixel(&px, 256, 64, 64);
    let twice = pixel(&px, 256, 112, 64);
    assert_eq!(alone, twice, "the overlap is more opaque than the rest");
    assert!(
        alone.0.abs_diff(128) <= 2,
        "half of white over black is {alone:?}"
    );
}

#[test]
fn glass_unchanged_after_trim() {
    let glassed = || {
        let mut scene = Scene::new();
        scene.set_background(Color::BLACK);
        scene.insert(Node::rect(Rect::new(0.0, 0.0, 128.0, 256.0)).fill(Color::WHITE));
        scene.insert(
            Node::rounded_rect(Rect::new(32.0, 32.0, 192.0, 192.0), 16.0)
                .fill(Color::TRANSPARENT)
                .backdrop(Blur::new(12.0)),
        );
        scene
    };

    let (gpu_lock, mut renderer) = renderer().split();
    let mut target = renderer.create_target(256, 256).unwrap();
    renderer.draw(&mut glassed(), &mut target).unwrap();
    let before = renderer.read_pixels(&mut target).unwrap();

    let mut plain = Scene::new();
    plain.set_background(Color::rgb(0.0, 0.0, 1.0));
    renderer.draw(&mut plain, &mut target).unwrap();
    drop(gpu_lock);
    std::thread::sleep(TRIM_AFTER);
    let _gpu_lock = rikka_test_renderer::lock_gpu();
    renderer.trim();

    renderer.draw(&mut glassed(), &mut target).unwrap();
    let after = renderer.read_pixels(&mut target).unwrap();
    assert!(
        before == after,
        "the glass drawn again after resting differs from the glass drawn first"
    );
}

#[test]
fn glass_and_plain_panes_alternate() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(128, 128).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(Node::rect(Rect::new(0.0, 0.0, 16.0, 16.0)).fill(Color::WHITE));
    let pane = scene.insert(
        Node::rounded_rect(Rect::new(32.0, 32.0, 64.0, 64.0), 8.0)
            .fill(Color::rgb(0.0, 0.0, 1.0))
            .backdrop(Blur::new(6.0)),
    );
    renderer.draw(&mut scene, &mut target).unwrap();

    scene
        .edit(pane)
        .clear_backdrop()
        .fill(Color::rgb(0.0, 1.0, 0.0));
    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&px, 128, 64, 64), (0, 255, 0, 255), "plain pane");

    scene
        .edit(pane)
        .backdrop(Blur::new(6.0))
        .fill(Color::rgb(1.0, 0.0, 0.0));
    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    let (red, green, blue, _) = pixel(&px, 128, 64, 64);
    assert!(
        red >= 250 && green <= 4 && blue <= 4,
        "glass pane is ({red}, {green}, {blue})"
    );
}

#[test]
fn wide_liquid_bar_updates_rim_under_inward_bulge() {
    const WIDTH: usize = 1200;
    const HEIGHT: usize = 320;
    const WHOLE: Rect = Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    const PANE: Rect = Rect::new(0.0, 0.0, 1200.0, 80.0);
    const PATCH: Rect = Rect::new(1078.0, 250.0, 12.0, 12.0);
    const GROUND: Color = Color::rgb(0.1, 0.2, 0.3);
    const PATCHED: Color = Color::rgb(0.95, 0.95, 0.1);

    fn desktop(patch_color: Color) -> (Scene, NodeId) {
        let mut scene = Scene::new();
        scene.insert(Node::rect(WHOLE).fill(GROUND));
        let patch = scene.insert(Node::rect(PATCH).fill(patch_color));
        scene.insert(
            Node::rounded_rect(PANE, 20.0)
                .fill(Color::WHITE.with_alpha(0.15))
                .backdrop(Blur::new(3.2))
                .lens(Lens::new()),
        );
        (scene, patch)
    }

    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH as u32, HEIGHT as u32).unwrap();
    let (mut scene, patch) = desktop(GROUND);
    renderer.draw(&mut scene, &mut target).unwrap();
    scene.edit(patch).fill(PATCHED);
    renderer.draw(&mut scene, &mut target).unwrap();
    let repaired = renderer.read_pixels(&mut target).unwrap();

    let (mut fresh, _) = desktop(PATCHED);
    renderer.draw(&mut fresh, &mut target).unwrap();
    let whole = renderer.read_pixels(&mut target).unwrap();

    assert_pane_matches("wide liquid bar rim", &repaired, &whole, WIDTH, PANE);
}

#[test]
fn overlapping_liquid_glass_propagates_stale_to_upper_pane() {
    const WIDTH: usize = 256;
    const HEIGHT: usize = 256;
    const WHOLE: Rect = Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    const LOWER: Rect = Rect::new(24.0, 24.0, 180.0, 180.0);
    const UPPER: Rect = Rect::new(80.0, 80.0, 140.0, 140.0);
    const PATCH: Rect = Rect::new(32.0, 32.0, 40.0, 40.0);

    fn desktop(patch_color: Color) -> (Scene, NodeId) {
        let mut scene = Scene::new();
        scene.insert(Node::rect(WHOLE).fill(Color::rgb(0.1, 0.1, 0.4)));
        let patch = scene.insert(Node::rect(PATCH).fill(patch_color));
        scene.insert(
            Node::rounded_rect(LOWER, 12.0)
                .fill(Color::WHITE.with_alpha(0.15))
                .backdrop(Blur::new(10.0))
                .z(1),
        );
        scene.insert(
            Node::rounded_rect(UPPER, 12.0)
                .fill(Color::WHITE.with_alpha(0.15))
                .backdrop(Blur::new(8.0))
                .z(2),
        );
        (scene, patch)
    }

    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH as u32, HEIGHT as u32).unwrap();
    let (mut scene, patch) = desktop(Color::rgb(0.1, 0.1, 0.4));
    renderer.draw(&mut scene, &mut target).unwrap();
    scene.edit(patch).fill(Color::rgb(0.95, 0.2, 0.1));
    renderer.draw(&mut scene, &mut target).unwrap();
    let repaired = renderer.read_pixels(&mut target).unwrap();

    let (mut fresh, _) = desktop(Color::rgb(0.95, 0.2, 0.1));
    renderer.draw(&mut fresh, &mut target).unwrap();
    let whole = renderer.read_pixels(&mut target).unwrap();

    assert_pane_matches("upper overlapping glass", &repaired, &whole, WIDTH, UPPER);
}

#[test]
fn one_pass_small_blur_cached_backdrop_renders_cleanly() {
    const WIDTH: usize = 128;
    const PANE: Rect = Rect::new(16.0, 16.0, 96.0, 96.0);
    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH as u32, WIDTH as u32).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.0, 0.0, 0.5));
    let patch =
        scene.insert(Node::rect(Rect::new(40.0, 40.0, 48.0, 48.0)).fill(Color::rgb(0.0, 0.0, 0.5)));
    scene.insert(
        Node::rounded_rect(PANE, 8.0)
            .fill(Color::WHITE.with_alpha(0.1))
            .backdrop(Blur::new(1.0)),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    scene.edit(patch).fill(Color::rgb(1.0, 0.0, 0.0));
    renderer.draw(&mut scene, &mut target).unwrap();
    let repaired = renderer.read_pixels(&mut target).unwrap();

    let mut fresh = Scene::new();
    fresh.set_background(Color::rgb(0.0, 0.0, 0.5));
    fresh.insert(Node::rect(Rect::new(40.0, 40.0, 48.0, 48.0)).fill(Color::rgb(1.0, 0.0, 0.0)));
    fresh.insert(
        Node::rounded_rect(PANE, 8.0)
            .fill(Color::WHITE.with_alpha(0.1))
            .backdrop(Blur::new(1.0)),
    );
    renderer.draw(&mut fresh, &mut target).unwrap();
    let whole = renderer.read_pixels(&mut target).unwrap();

    assert_pane_matches("1-pass cached blur", &repaired, &whole, WIDTH, PANE);
}

#[test]
fn shrunk_backdrop_pane_reblurs_cleanly_on_shrink_and_regrow() {
    const WIDTH: usize = 512;
    const HEIGHT: usize = 384;
    const LARGE: Rect = Rect::new(16.0, 16.0, 480.0, 352.0);
    const SMALL: Rect = Rect::new(16.0, 16.0, 128.0, 96.0);
    const PATCH: Rect = Rect::new(320.0, 220.0, 80.0, 80.0);

    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH as u32, HEIGHT as u32).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::rgb(0.1, 0.1, 0.1));
    let patch = scene.insert(Node::rect(PATCH).fill(Color::rgb(0.1, 0.1, 0.1)));
    let pane = scene.insert(
        Node::rounded_rect(LARGE, 8.0)
            .fill(Color::WHITE.with_alpha(0.15))
            .backdrop(Blur::new(6.0)),
    );
    renderer.draw(&mut scene, &mut target).unwrap();

    scene.edit(pane).rect(SMALL);
    renderer.draw(&mut scene, &mut target).unwrap();

    scene.edit(patch).fill(Color::rgb(0.1, 0.9, 0.2));
    renderer.draw(&mut scene, &mut target).unwrap();

    scene.edit(pane).rect(LARGE);
    renderer.draw(&mut scene, &mut target).unwrap();
    let regrown = renderer.read_pixels(&mut target).unwrap();

    let mut fresh = Scene::new();
    fresh.set_background(Color::rgb(0.1, 0.1, 0.1));
    fresh.insert(Node::rect(PATCH).fill(Color::rgb(0.1, 0.9, 0.2)));
    fresh.insert(
        Node::rounded_rect(LARGE, 8.0)
            .fill(Color::WHITE.with_alpha(0.15))
            .backdrop(Blur::new(6.0)),
    );
    renderer.draw(&mut fresh, &mut target).unwrap();
    let whole = renderer.read_pixels(&mut target).unwrap();

    assert_pane_matches("regrown glass", &regrown, &whole, WIDTH, LARGE);
}

#[test]
fn invisible_layer_fades_in_on_gpu() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(64, 64).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    let layer = scene.create_layer();
    scene.set_layer(layer, None, 0.0);
    scene.insert(
        Node::rect(Rect::new(8.0, 8.0, 48.0, 48.0))
            .layer(layer)
            .fill(Color::rgb(1.0, 0.0, 0.0)),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    let hidden = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&hidden, 64, 32, 32), (0, 0, 0, 255));

    scene.set_layer(layer, None, 1.0);
    renderer.draw(&mut scene, &mut target).unwrap();
    let shown = renderer.read_pixels(&mut target).unwrap();
    assert_eq!(pixel(&shown, 64, 32, 32), (255, 0, 0, 255));
}

#[test]
fn full_circle_arc_has_no_seam_gap() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(64, 64).unwrap();
    let mut scene = Scene::new();
    scene.set_background(Color::BLACK);
    scene.insert(
        Node::arc(Point::new(32.0, 32.0), 24.0, 0.0, std::f32::consts::TAU)
            .stroke(Stroke::new(6.0, Color::WHITE)),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    let px = renderer.read_pixels(&mut target).unwrap();
    for (x, y) in [(53, 32), (10, 32), (32, 10), (32, 53)] {
        let (r, g, b, _) = pixel(&px, 64, x, y);
        assert!(
            r > 240 && g > 240 && b > 240,
            "full arc seam at ({x}, {y}) was ({r}, {g}, {b})"
        );
    }
}

#[test]
fn cutout_and_clip_side_table_match_expected_pixels() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(64, 64).unwrap();
    for immediate in [false, true] {
        let mut scene = if immediate {
            Scene::immediate()
        } else {
            Scene::new()
        };
        scene.set_background(Color::BLACK);
        scene.insert(
            Node::rect(Rect::new(0.0, 0.0, 64.0, 64.0))
                .fill(Color::rgb(0.0, 1.0, 0.0))
                .clip(RoundedRect::rect(Rect::new(8.0, 8.0, 48.0, 48.0)))
                .cut_out(RoundedRect::rect(Rect::new(24.0, 24.0, 16.0, 16.0))),
        );
        renderer.draw(&mut scene, &mut target).unwrap();
        let px = renderer.read_pixels(&mut target).unwrap();
        assert_eq!(pixel(&px, 64, 4, 4), (0, 0, 0, 255), "outside clip");
        assert_eq!(pixel(&px, 64, 16, 16), (0, 255, 0, 255), "inside clip");
        assert_eq!(pixel(&px, 64, 32, 32), (0, 0, 0, 255), "inside cutout");
    }
}
