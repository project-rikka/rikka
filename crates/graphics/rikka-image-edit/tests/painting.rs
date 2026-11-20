use crate::support::{at, close, developed, flat};
use rikka_image_edit::{
    Brush, BrushStroke, Cap, FillRule, Gradient, GradientInk, GradientShape, Ink, Join, Marking,
    Marks, Path, PathMark, Picture, Recipe, StrokeStyle,
};
use rikka_types::{Color, Point};

const WHITE: [u8; 4] = [255, 255, 255, 255];

fn painted(picture: &Picture, marks: impl IntoIterator<Item = Marking>) -> Picture {
    developed(picture, Recipe::new().then(Marks::new(marks)))
}

fn dot(brush: Brush, at: Point) -> Marking {
    BrushStroke::new(brush, Ink::Paint(Color::BLACK), at).into()
}

fn stroke(brush: Brush, ink: Ink, points: &[(f32, f32)]) -> Marking {
    let point = |(x, y): (f32, f32)| Point::new(x, y);
    BrushStroke::from_points(brush, ink, points.iter().copied().map(point))
        .expect("stroke has points")
        .into()
}

fn red(picture: &Picture, x: u32, y: u32) -> u8 {
    at(picture, x, y)[0]
}

#[test]
fn soft_brush_falls_off_radially() {
    let white = flat(64, 64, WHITE);
    let soft = painted(
        &white,
        [dot(
            Brush {
                hardness: 0.0,
                ..Brush::new(40.0)
            },
            Point::new(32.5, 32.5),
        )],
    );

    assert!(red(&soft, 32, 32) < 8, "{}", red(&soft, 32, 32));
    assert!(close(red(&soft, 42, 32), 128, 3), "{}", red(&soft, 42, 32));
    assert_eq!(red(&soft, 53, 32), 255, "pixel beyond brush rim");
    let ring: Vec<u8> = [(42, 32), (22, 32), (32, 42), (32, 22)]
        .into_iter()
        .map(|(x, y)| red(&soft, x, y))
        .collect();
    assert!(
        ring.iter().all(|value| close(*value, ring[0], 1)),
        "{ring:?}"
    );
}

#[test]
fn overlapping_stroke_capped_at_opacity() {
    let white = flat(80, 40, WHITE);
    let brush = Brush {
        hardness: 0.6,
        opacity: 0.5,
        ..Brush::new(16.0)
    };
    let once = painted(
        &white,
        [stroke(
            brush,
            Ink::Paint(Color::BLACK),
            &[(10.0, 20.0), (70.0, 20.0)],
        )],
    );
    let scrubbed = painted(
        &white,
        [stroke(
            brush,
            Ink::Paint(Color::BLACK),
            &[
                (10.0, 20.0),
                (70.0, 20.0),
                (10.0, 21.0),
                (70.0, 19.0),
                (40.0, 20.0),
            ],
        )],
    );

    assert!(close(red(&once, 40, 20), 128, 2), "{}", red(&once, 40, 20));
    assert!(
        close(red(&scrubbed, 40, 20), 128, 2),
        "{}",
        red(&scrubbed, 40, 20)
    );
    let along: Vec<u8> = (20..60).map(|x| red(&once, x, 25)).collect();
    let (low, high) = (
        along.iter().min().copied().unwrap_or_default(),
        along.iter().max().copied().unwrap_or_default(),
    );
    assert!(high - low <= 1, "no beading along edge: {along:?}");
}

#[test]
fn eraser_removes_alpha_by_opacity() {
    let red_paper = flat(40, 40, [255, 0, 0, 255]);
    let erase = |opacity: f32| {
        stroke(
            Brush {
                opacity,
                ..Brush::new(10.0)
            },
            Ink::Erase {
                background: Color::WHITE,
            },
            &[(5.0, 20.0), (35.0, 20.0)],
        )
    };
    let gone = painted(&red_paper, [erase(1.0)]);
    let halved = painted(&red_paper, [erase(0.5)]);

    assert!(gone.has_alpha());
    assert_eq!(at(&gone, 20, 20)[3], 0);
    assert_eq!(at(&gone, 20, 5), [255, 0, 0, 255]);
    assert!(close(at(&halved, 20, 20)[3], 128, 2));
    assert_eq!(at(&halved, 20, 20)[0], 255, "color preserved");
}

fn gradient(shape: GradientShape, from: (f32, f32), to: (f32, f32)) -> Recipe {
    Recipe::new().then(Gradient::new(
        shape,
        Point::new(from.0, from.1),
        Point::new(to.0, to.1),
        GradientInk::Colors {
            from: Color::BLACK,
            to: Color::WHITE,
        },
    ))
}

#[test]
fn linear_gradient_interpolates_stops() {
    let paper = flat(101, 4, [255, 0, 0, 255]);
    let ramp = developed(
        &paper,
        gradient(GradientShape::Linear, (0.5, 0.0), (100.5, 0.0)),
    );

    assert_eq!(at(&ramp, 0, 1), [0, 0, 0, 255]);
    assert_eq!(at(&ramp, 100, 1), [255, 255, 255, 255]);
    assert!(close(red(&ramp, 50, 1), 128, 2), "{}", red(&ramp, 50, 1));
    assert!(close(red(&ramp, 25, 1), 64, 2), "{}", red(&ramp, 25, 1));
}

#[test]
fn shallow_gradient_is_dithered() {
    let paper = flat(256, 64, WHITE);
    let shallow = developed(
        &paper,
        Recipe::new().then(Gradient::new(
            GradientShape::Linear,
            Point::new(0.0, 0.0),
            Point::new(256.0, 0.0),
            GradientInk::Colors {
                from: Color::rgba8(100, 100, 100, 255),
                to: Color::rgba8(102, 102, 102, 255),
            },
        )),
    );
    let column: Vec<u8> = (0..64).map(|y| red(&shallow, 64, y)).collect();
    let mean = column.iter().map(|value| f32::from(*value)).sum::<f32>() / 64.0;

    assert!(column.contains(&100) && column.contains(&101), "{column:?}");
    assert!((mean - 100.5).abs() < 0.3, "{mean}");
}

fn star() -> Path {
    let points = (0..5).map(|at| {
        let turn = (at * 2) as f32 / 5.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        Point::new(32.0 + 28.0 * turn.cos(), 32.0 + 28.0 * turn.sin())
    });
    Path::polygon(points)
}

#[test]
fn fill_rule_controls_self_intersection() {
    let white = flat(64, 64, WHITE);
    let filled = |rule| {
        painted(
            &white,
            [PathMark::new(star()).filled(Color::BLACK, rule).into()],
        )
    };
    let nonzero = filled(FillRule::NonZero);
    let even_odd = filled(FillRule::EvenOdd);

    assert_eq!(red(&nonzero, 32, 32), 0);
    assert_eq!(red(&even_odd, 32, 32), 255);
    assert_eq!(red(&nonzero, 32, 8), 0, "star point");
    assert_eq!(red(&even_odd, 32, 8), 0);
    assert_eq!(red(&nonzero, 4, 60), 255);
}

#[test]
fn filled_edge_uses_area_coverage() {
    let white = flat(32, 32, WHITE);
    let square = Path::polygon([
        Point::new(4.0, 4.0),
        Point::new(20.5, 4.0),
        Point::new(20.5, 20.0),
        Point::new(4.0, 20.0),
    ]);
    let smooth = painted(
        &white,
        [PathMark::new(square.clone())
            .filled(Color::BLACK, FillRule::NonZero)
            .into()],
    );
    let jagged = painted(
        &white,
        [PathMark::new(square)
            .filled(Color::BLACK, FillRule::NonZero)
            .antialiased(false)
            .into()],
    );

    assert_eq!(red(&smooth, 19, 10), 0);
    assert!(
        close(red(&smooth, 20, 10), 128, 2),
        "{}",
        red(&smooth, 20, 10)
    );
    assert_eq!(red(&smooth, 21, 10), 255);
    assert!(matches!(red(&jagged, 20, 10), 0 | 255));
}

fn bent(join: Join) -> Picture {
    let white = flat(64, 64, WHITE);
    let path = Path::polyline([
        Point::new(10.0, 40.0),
        Point::new(40.0, 40.0),
        Point::new(40.0, 10.0),
    ]);
    painted(
        &white,
        [PathMark::new(path)
            .stroked(Color::BLACK, StrokeStyle::new(10.0).join(join))
            .into()],
    )
}

#[test]
fn joins_miter_round_bevel() {
    let mitre = bent(Join::Miter);
    let round = bent(Join::Round);
    let bevel = bent(Join::Bevel);

    assert_eq!(red(&mitre, 44, 44), 0);
    assert_eq!(red(&round, 44, 44), 255);
    assert_eq!(red(&bevel, 44, 44), 255);
    assert!(red(&round, 43, 42) < 8, "{}", red(&round, 43, 42));
    assert_eq!(red(&bevel, 43, 42), 255);
    assert_eq!(red(&bevel, 41, 41), 0);
    assert_eq!(red(&mitre, 25, 40), 0, "pixel on line");
    assert_eq!(red(&mitre, 25, 46), 255, "pixel beside corner");
}

fn capped(cap: Cap) -> Picture {
    let white = flat(48, 40, WHITE);
    let path = Path::polyline([Point::new(10.0, 20.0), Point::new(30.0, 20.0)]);
    painted(
        &white,
        [PathMark::new(path)
            .stroked(Color::BLACK, StrokeStyle::new(6.0).cap(cap))
            .into()],
    )
}

#[test]
fn caps_butt_round_square() {
    let butt = capped(Cap::Butt);
    let round = capped(Cap::Round);
    let square = capped(Cap::Square);

    assert_eq!(red(&butt, 29, 20), 0);
    assert_eq!(red(&butt, 31, 20), 255);
    assert_eq!(red(&round, 31, 20), 0);
    assert!(red(&round, 32, 22) > 230, "{}", red(&round, 32, 22));
    assert_eq!(red(&square, 32, 22), 0);
    assert_eq!(red(&square, 34, 20), 255);
}
