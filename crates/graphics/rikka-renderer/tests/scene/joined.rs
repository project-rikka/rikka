use rikka_renderer::{Blur, Joined, Node, Scene};
use rikka_test_renderer::TestRenderer;
use rikka_types::{Color, CornerRadii, Lens, Rect, RoundedRect};

const WIDTH: usize = 512;
const HEIGHT: usize = 320;
const HOST: Rect = Rect::new(0.0, 0.0, 512.0, 40.0);
const LIMB: Rect = Rect::new(156.0, 40.0, 200.0, 160.0);
const FAR: CornerRadii = CornerRadii::new(0.0, 0.0, 16.0, 16.0);

fn renderer() -> TestRenderer {
    rikka_test_renderer::renderer("rikka-renderer-test")
        .expect("a Vulkan 1.3 device is required for these tests")
}

fn ramp(scene: &mut Scene) {
    for row in 0..HEIGHT {
        let share = row as f32 / HEIGHT as f32;
        scene.insert(
            Node::rect(Rect::new(0.0, row as f32, WIDTH as f32, 1.0)).fill(Color::rgb(
                share,
                0.5,
                1.0 - share,
            )),
        );
    }
}

fn glass(shape: RoundedRect, joined: Option<Joined>) -> Node {
    let node = Node::rounded_rect(shape.rect, shape.radii)
        .fill(Color::TRANSPARENT)
        .backdrop(Blur::with_radius(4.0))
        .lens(Lens::new());
    match joined {
        Some(joined) => node.joined(joined),
        None => node,
    }
}

fn drawn(renderer: &mut TestRenderer, joins: bool) -> Vec<u8> {
    let host = RoundedRect::rect(HOST);
    let limb = RoundedRect::new(LIMB, FAR);
    let joined = Joined::new(host, limb).filter(|_| joins);
    let mut scene = Scene::new();
    ramp(&mut scene);
    scene.insert(glass(host, joined));
    scene.insert(glass(limb, joined));
    for (square, hollow) in joined.iter().flat_map(Joined::fillets).flatten() {
        scene.insert(glass(RoundedRect::rect(square), joined).cut_out(hollow));
    }
    let mut target = renderer.create_target(WIDTH as u32, HEIGHT as u32).unwrap();
    renderer.draw(&mut scene, &mut target).unwrap();
    renderer.read_pixels(&mut target).unwrap()
}

fn rows_apart(pixels: &[u8], above: usize) -> f32 {
    let across = (LIMB.min_x() as usize + 24)..(LIMB.max_x() as usize - 24);
    let count = across.len() as f32;
    across
        .map(|x| {
            let at = |y: usize| &pixels[(y * WIDTH + x) * 4..][..3];
            at(above)
                .iter()
                .zip(at(above + 1))
                .map(|(a, b)| f32::from(a.abs_diff(*b)))
                .sum::<f32>()
        })
        .sum::<f32>()
        / count
}

#[test]
fn joined_glass_refracts_as_one_body() {
    let mut renderer = renderer();
    let seam = HOST.max_y() as usize - 1;
    let within = (LIMB.min_y() + LIMB.height() * 0.5) as usize;

    let joined = drawn(&mut renderer, true);
    let calm = rows_apart(&joined, within);
    let across = rows_apart(&joined, seam);
    assert!(
        across <= calm * 2.0 + 3.0,
        "joined glass shows no seam: {across} across it against {calm} within"
    );

    let apart = drawn(&mut renderer, false);
    let parted = rows_apart(&apart, seam);
    assert!(
        parted > across * 10.0 + 12.0,
        "two separate panes show their seam ({parted} against {across} joined, {calm} within)"
    );
}

#[test]
fn joined_requires_square_limb_on_straight_side() {
    let host = RoundedRect::rect(HOST);
    let hanging = RoundedRect::new(LIMB, FAR);
    let shifted = |dx: f32, dy: f32| {
        RoundedRect::new(
            Rect::new(LIMB.x() + dx, LIMB.y() + dy, LIMB.width(), LIMB.height()),
            FAR,
        )
    };
    let above = RoundedRect::new(
        Rect::new(156.0, -160.0, 200.0, 160.0),
        CornerRadii::new(16.0, 16.0, 0.0, 0.0),
    );
    let beside = RoundedRect::new(
        Rect::new(512.0, 8.0, 30.0, 24.0),
        CornerRadii::new(0.0, 4.0, 4.0, 0.0),
    );
    let cases = [
        ("hanging below", host, hanging, Some(LIMB)),
        (
            "half a pixel short of it",
            host,
            shifted(0.0, 0.5),
            Some(Rect::new(156.0, 40.0, 200.0, 160.5)),
        ),
        ("standing above", host, above, Some(above.rect)),
        ("beside", host, beside, Some(beside.rect)),
        ("a gap away", host, shifted(0.0, 4.0), None),
        ("fillets past the host", host, shifted(-148.0, 0.0), None),
        (
            "rounded where it meets",
            host,
            RoundedRect::new(LIMB, CornerRadii::uniform(8.0)),
            None,
        ),
        (
            "host rounded where it meets",
            RoundedRect::new(HOST, CornerRadii::uniform(8.0)),
            hanging,
            None,
        ),
        ("the host from its limb", hanging, host, None),
    ];
    for (name, host, limb, joins) in cases {
        let joined = Joined::new(host, limb);
        assert_eq!(joined.map(|joined| joined.limb().rect), joins, "{name}");
    }
}
