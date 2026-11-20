use rikka_renderer::{Node, OffscreenTarget, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_text::{IconSpec, TextPaint, TextSpec};
use rikka_types::{Color, Rect};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 200;

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-glyph-curves")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn draw(renderer: &mut TestRenderer, target: &mut OffscreenTarget, scene: &mut Scene) -> Vec<u8> {
    renderer.draw(scene, &mut *target).unwrap();
    renderer.read_pixels(target).unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Subject {
    AtlasText,
    CurveText,
    Icon,
}

impl Subject {
    fn node(self, ink: Color) -> Node {
        match self {
            Subject::AtlasText => Node::text(
                Rect::new(8.0, 8.0, 600.0, 40.0),
                TextSpec::new("Coverage ignores ink", 15.0).layout(Some(600.0)),
            ),
            Subject::CurveText => Node::text(
                Rect::new(8.0, 8.0, 620.0, 180.0),
                TextSpec::new("Ink", 120.0).layout(Some(620.0)),
            ),
            Subject::Icon => Node::icon(
                Rect::new(8.0, 8.0, 48.0, 48.0),
                IconSpec::new('\u{e88a}', 40.0),
            ),
        }
        .fill(ink)
    }
}

#[test]
fn curve_coverage_independent_of_color() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH, HEIGHT).unwrap();
    let inks = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0].map(|level| Color::rgb(level, level, level));
    for subject in [Subject::AtlasText, Subject::CurveText, Subject::Icon] {
        let coverages: Vec<Vec<u8>> = inks
            .iter()
            .map(|&ink| {
                let mut scene = Scene::new();
                scene.insert(subject.node(ink));
                let pixels = draw(&mut renderer, &mut target, &mut scene);
                pixels.as_chunks::<4>().0.iter().map(|p| p[3]).collect()
            })
            .collect();
        let darkest = &coverages[0];
        let partial = darkest.iter().filter(|&&a| a > 0 && a < 255).count();
        let solid = darkest.iter().filter(|&&a| a == 255).count();
        assert!(partial > 50, "{subject:?}: {partial} edge pixels");
        assert!(solid > 20, "{subject:?}: {solid} solid pixels");
        for (ink, coverage) in inks.iter().zip(&coverages) {
            assert!(
                coverage == darkest,
                "{subject:?}: coverage under {ink:?} differs from black ink"
            );
        }
    }
}

#[test]
fn zoomed_text_settles_to_static_pixels() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH, HEIGHT).unwrap();
    let layout = TextSpec::new("Zoom settles 東京", 15.0).layout(Some(400.0));
    let at = |scale: f32| TextPaint::new(layout.clone()).scaled(scale);
    let rect = Rect::new(8.0, 8.0, 600.0, 60.0);

    let mut rested = Scene::new();
    rested.insert(Node::text(rect, at(1.5)).fill(Color::WHITE));
    let expected = draw(&mut renderer, &mut target, &mut rested);

    let mut zoomed = Scene::new();
    let text = zoomed.insert(Node::text(rect, at(1.0)).fill(Color::WHITE));
    draw(&mut renderer, &mut target, &mut zoomed);
    for scale in [1.1, 1.2, 1.3, 1.4, 1.5] {
        zoomed.edit(text).text(at(scale));
        renderer.draw(&mut zoomed, &mut target).unwrap();
    }
    assert!(zoomed.is_animating(), "a zoom keeps frames coming");
    let mut frames = 0;
    while zoomed.is_animating() {
        assert!(frames < 120, "the zoom settled within 120 frames");
        renderer.draw(&mut zoomed, &mut target).unwrap();
        frames += 1;
    }
    let settled = draw(&mut renderer, &mut target, &mut zoomed);
    assert!(
        settled == expected,
        "settled text matches text drawn at rest"
    );
}

#[test]
fn zoom_first_frame_matches_final_layout() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH, HEIGHT).unwrap();
    let layout = TextSpec::new("First zoomed frame", 15.0).layout(Some(400.0));
    let at = |scale: f32| TextPaint::new(layout.clone()).scaled(scale);
    let rect = Rect::new(8.0, 8.0, 600.0, 60.0);

    let mut zoomed = Scene::new();
    let text = zoomed.insert(Node::text(rect, at(1.0)).fill(Color::WHITE));
    draw(&mut renderer, &mut target, &mut zoomed);
    zoomed.edit(text).text(at(1.5));
    let first = Ink::measure(&draw(&mut renderer, &mut target, &mut zoomed));

    let mut rested = Scene::new();
    rested.insert(Node::text(rect, at(1.5)).fill(Color::WHITE));
    let expected = Ink::measure(&draw(&mut renderer, &mut target, &mut rested));

    for (edge, (shown, rests)) in first.bounds.iter().zip(expected.bounds).enumerate() {
        assert!(
            shown.abs_diff(rests) <= 2,
            "edge {edge} of the first frame's ink: {shown} against {rests}"
        );
    }
    let ratio = first.coverage as f64 / expected.coverage as f64;
    assert!(
        (0.85..1.15).contains(&ratio),
        "the first frame covers {ratio} of the rested text"
    );
}

struct Ink {
    bounds: [u32; 4],
    coverage: u64,
}

impl Ink {
    fn measure(pixels: &[u8]) -> Ink {
        let mut bounds = [u32::MAX, u32::MAX, 0, 0];
        let mut coverage = 0;
        for (at, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] == 0 {
                continue;
            }
            let (x, y) = ((at as u32) % WIDTH, (at as u32) / WIDTH);
            bounds = [
                bounds[0].min(x),
                bounds[1].min(y),
                bounds[2].max(x),
                bounds[3].max(y),
            ];
            coverage += u64::from(pixel[3]);
        }
        Ink { bounds, coverage }
    }
}

#[test]
fn curve_store_growth_preserves_glyphs() {
    let mut renderer = renderer();
    let mut target = renderer.create_target(WIDTH, HEIGHT).unwrap();
    let mut scene = Scene::new();
    scene.insert(Subject::CurveText.node(Color::WHITE));
    let before = draw(&mut renderer, &mut target, &mut scene);

    let crowd: String = ('\u{100}'..'\u{500}').collect();
    let crowd = scene.insert(
        Node::text(
            Rect::new(0.0, 400.0, 100_000.0, 200.0),
            TextSpec::new(crowd, 80.0).layout(Some(100_000.0)),
        )
        .fill(Color::WHITE),
    );
    renderer.draw(&mut scene, &mut target).unwrap();
    scene.remove(crowd);
    scene.damage_all();
    let after = draw(&mut renderer, &mut target, &mut scene);
    assert!(
        after == before,
        "curves placed before the store grew still draw"
    );
}
