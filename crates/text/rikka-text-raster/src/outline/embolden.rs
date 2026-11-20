use rikka_types::Point;

use crate::outline::path::Outline;

const MITER_FLOOR: f32 = 0.25;

pub(crate) fn outward_miters(outline: &Outline, normals: &mut Vec<Point>, miters: &mut Vec<Point>) {
    miters.clear();
    let area = signed_area(outline);
    if area == 0.0 {
        return;
    }
    let outward = if area > 0.0 { 1.0 } else { -1.0 };
    miters.resize(outline.path().points().len(), Point::default());
    normals.clear();
    normals.resize(outline.path().points().len(), Point::default());
    for contour in outline.path().contours() {
        let points = &outline.path().points()[contour.clone()];
        contour_miters(
            points,
            &mut normals[contour.clone()],
            &mut miters[contour],
            outward,
        );
    }
}

pub(crate) fn embolden(outline: &mut Outline, miters: &[Point], amount: f32) {
    for (point, miter) in outline.path_mut().points_mut().iter_mut().zip(miters) {
        *point = Point::new(point.x + miter.x * amount, point.y + miter.y * amount);
    }
}

fn signed_area(outline: &Outline) -> f32 {
    let mut area = 0.0f64;
    for contour in outline.path().contours() {
        let points = &outline.path().points()[contour];
        let Some(&last) = points.last() else {
            continue;
        };
        let mut previous = last;
        for &point in points {
            area += f64::from(previous.x) * f64::from(point.y)
                - f64::from(point.x) * f64::from(previous.y);
            previous = point;
        }
    }
    area as f32
}

fn contour_miters(points: &[Point], normals: &mut [Point], miters: &mut [Point], outward: f32) {
    if points.len() < 2 {
        return;
    }
    let next_points = points[1..].iter().chain(&points[..1]);
    for ((normal, from), to) in normals.iter_mut().zip(points).zip(next_points) {
        *normal = edge_normal(*from, *to);
    }
    let Some(mut outgoing) = normals.iter().copied().find(is_edge) else {
        return;
    };
    for (normal, slot) in normals.iter().zip(miters.iter_mut()).rev() {
        if is_edge(normal) {
            outgoing = *normal;
        }
        *slot = outgoing;
    }
    let Some(mut incoming) = normals.iter().rev().copied().find(is_edge) else {
        return;
    };
    for (normal, miter) in normals.iter().zip(miters.iter_mut()) {
        let outgoing = *miter;
        let cosine = incoming.x * outgoing.x + incoming.y * outgoing.y;
        let scale = 1.0 / (1.0 + cosine).max(MITER_FLOOR);
        *miter = Point::new(
            (incoming.x + outgoing.x) * scale * outward,
            (incoming.y + outgoing.y) * scale * outward,
        );
        if is_edge(normal) {
            incoming = *normal;
        }
    }
}

#[inline]
fn edge_normal(from: Point, to: Point) -> Point {
    let (x, y) = (to.x - from.x, to.y - from.y);
    if x == 0.0 && y == 0.0 {
        return Point::new(f32::NAN, f32::NAN);
    }
    let length = (x * x + y * y).sqrt();
    Point::new(y / length, -(x / length))
}

#[inline]
fn is_edge(normal: &Point) -> bool {
    !normal.x.is_nan()
}
