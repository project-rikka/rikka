use std::f32::consts::PI;

use rikka_types::{Color, Point, Rect};
use rikka_vector::{Canvas, Cap, Join, Paint, Path, Stroke, Transform};

const INK: Paint<'static> = Paint::Solid(Color::rgba(0.0, 0.0, 0.0, 1.0));

fn stroked(path: &Path, stroke: &Stroke<'_>, transform: Transform) -> f32 {
    let mut canvas = Canvas::new();
    canvas.begin(64, 64);
    canvas
        .stroke(path, &transform, stroke, &INK)
        .expect("strokes");
    let alpha: u32 = canvas
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pixel| u32::from(pixel[3]))
        .sum();
    alpha as f32 / 255.0
}

fn polyline(points: &[(f32, f32)], closed: bool) -> Path {
    let mut path = Path::new();
    for (index, &(x, y)) in points.iter().enumerate() {
        match index {
            0 => path.move_to(Point::new(x, y)),
            _ => path.line_to(Point::new(x, y)),
        }
    }
    if closed {
        path.close();
    }
    path
}

fn assert_round_area(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.35 + expected * 0.02,
        "{what}: {actual} instead of {expected}"
    );
}

fn assert_area(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() < 0.35 + expected * 0.004,
        "{what}: {actual} instead of {expected}"
    );
}

#[test]
fn caps_extend_open_ends() {
    let line = polyline(&[(12.3, 20.6), (40.3, 20.6)], false);
    for (cap, area) in [
        (Cap::Butt, 28.0 * 6.0),
        (Cap::Square, 34.0 * 6.0),
        (Cap::Round, 28.0 * 6.0 + PI * 9.0),
    ] {
        let stroke = Stroke::new(6.0).cap(cap);
        assert_area(
            stroked(&line, &stroke, Transform::IDENTITY),
            area,
            &format!("{cap:?}"),
        );
    }
    let point = polyline(&[(30.0, 30.0), (30.0, 30.0)], false);
    assert_round_area(
        stroked(
            &point,
            &Stroke::new(6.0).cap(Cap::Round),
            Transform::IDENTITY,
        ),
        PI * 9.0,
        "dot",
    );
    assert_area(
        stroked(&point, &Stroke::new(6.0), Transform::IDENTITY),
        0.0,
        "butt point",
    );
}

#[test]
fn joins_shape_corners() {
    let corner = polyline(&[(10.0, 40.0), (30.0, 40.0), (30.0, 20.0)], false);
    let arms = 2.0 * 20.0 * 8.0 - 16.0;
    for (stroke, area) in [
        (Stroke::new(8.0).join(Join::Miter), arms + 16.0),
        (Stroke::new(8.0).join(Join::Bevel), arms + 8.0),
        (Stroke::new(8.0).join(Join::Round), arms + PI * 4.0),
        (
            Stroke::new(8.0).join(Join::Miter).miter_limit(1.2),
            arms + 8.0,
        ),
    ] {
        assert_area(
            stroked(&corner, &stroke, Transform::IDENTITY),
            area,
            &format!("{:?} limit {}", stroke.join, stroke.miter_limit),
        );
    }
}

#[test]
fn closed_paths_join_all_around() {
    let square = polyline(
        &[(16.0, 16.0), (48.0, 16.0), (48.0, 48.0), (16.0, 48.0)],
        true,
    );
    assert_area(
        stroked(&square, &Stroke::new(4.0), Transform::IDENTITY),
        36.0 * 36.0 - 28.0 * 28.0,
        "square",
    );
    let mut circle = Path::new();
    circle.add_ellipse(Point::new(32.0, 32.0), Point::new(20.0, 20.0));
    assert_area(
        stroked(
            &circle,
            &Stroke::new(6.0).join(Join::Bevel),
            Transform::IDENTITY,
        ),
        PI * (23.0 * 23.0 - 17.0 * 17.0),
        "circle",
    );
    let mut rounded = Path::new();
    rounded.add_rounded_rect(Rect::new(10.0, 10.0, 40.0, 30.0), Point::new(8.0, 8.0));
    let length = 2.0 * (24.0 + 14.0) + 2.0 * PI * 8.0;
    assert_area(
        stroked(&rounded, &Stroke::new(2.0), Transform::IDENTITY),
        length * 2.0,
        "rounded rectangle",
    );
}

#[test]
fn dashes_cut_lines_and_wrap_closed_paths() {
    let line = polyline(&[(8.0, 30.5), (56.0, 30.5)], false);
    for (dashes, offset, area) in [
        (&[8.0, 8.0][..], 0.0, 3.0 * 8.0 * 4.0),
        (&[8.0, 8.0][..], 4.0, (4.0 + 8.0 + 8.0 + 4.0) * 4.0),
        (&[6.0][..], 0.0, 4.0 * 6.0 * 4.0),
        (&[0.0, 0.0][..], 0.0, 48.0 * 4.0),
    ] {
        let stroke = Stroke::new(4.0).dashed(dashes, offset);
        assert_area(
            stroked(&line, &stroke, Transform::IDENTITY),
            area,
            &format!("{dashes:?} from {offset}"),
        );
    }
    let square = polyline(
        &[(16.0, 16.0), (48.0, 16.0), (48.0, 48.0), (16.0, 48.0)],
        true,
    );
    let wrapped = Stroke::new(4.0).dashed(&[16.0, 16.0], 8.0);
    assert_area(
        stroked(&square, &wrapped, Transform::IDENTITY),
        4.0 * 16.0 * 4.0,
        "dashes around corners",
    );
}

#[test]
fn strokes_follow_the_transform() {
    let line = polyline(&[(4.0, 8.0), (14.0, 8.0)], false);
    let area = stroked(&line, &Stroke::new(2.0), Transform::scale(3.0, 2.0));
    assert_area(area, 30.0 * 4.0, "scaled stroke");
}

#[test]
fn retraced_lines_cover_their_width_once() {
    for (points, closed) in [
        (&[(10.5, 8.0), (10.5, 28.0)][..], true),
        (&[(10.5, 8.0), (10.5, 28.0), (10.5, 8.0)][..], false),
    ] {
        let area = stroked(
            &polyline(points, closed),
            &Stroke::new(3.0),
            Transform::IDENTITY,
        );
        assert_area(area, 20.0 * 3.0, "line drawn there and back");
    }
}
