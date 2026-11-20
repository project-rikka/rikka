use crate::glyf::glyph::ON_CURVE;
use crate::outline::sink::OutlineSink;
use rikka_types::Point;

pub(crate) fn emit_contours(
    points: &[Point],
    flags: &[u8],
    contour_ends: &[usize],
    sink: &mut impl OutlineSink,
) {
    let mut start = 0;
    for &end in contour_ends {
        if end < start {
            continue;
        }
        let (Some(contour), Some(contour_flags)) =
            (points.get(start..=end), flags.get(start..=end))
        else {
            return;
        };
        emit_contour(contour, contour_flags, sink);
        start = end + 1;
    }
}

fn emit_contour(points: &[Point], flags: &[u8], sink: &mut impl OutlineSink) {
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
        return;
    };
    let on_curve = |index: usize| flags.get(index).is_some_and(|flag| flag & ON_CURVE != 0);
    let last_index = points.len() - 1;
    let (start, skip_first, skip_last) = if on_curve(0) {
        (first, true, false)
    } else if on_curve(last_index) {
        (last, false, true)
    } else {
        (midpoint(last, first), false, false)
    };
    sink.move_to(start.x, start.y);
    let mut current = start;
    let mut pending: Option<Point> = None;
    let begin = usize::from(skip_first);
    let end = if skip_last { last_index } else { points.len() };
    for (index, &point) in points.iter().enumerate().take(end).skip(begin) {
        if on_curve(index) {
            match pending.take() {
                Some(control) => sink.quad_to(control.x, control.y, point.x, point.y),
                None => sink.line_to(point.x, point.y),
            }
            current = point;
        } else {
            if let Some(control) = pending {
                let middle = midpoint(control, point);
                sink.quad_to(control.x, control.y, middle.x, middle.y);
                current = middle;
            }
            pending = Some(point);
        }
    }
    match pending {
        Some(control) => sink.quad_to(control.x, control.y, start.x, start.y),
        None if current != start => sink.line_to(start.x, start.y),
        None => {}
    }
    sink.close();
}

#[inline]
fn midpoint(one: Point, other: Point) -> Point {
    Point::new((one.x + other.x) * 0.5, (one.y + other.y) * 0.5)
}
