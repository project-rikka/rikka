use rikka_types::{Rect, Size};

use crate::area::{Area, GAP};

const SLACK: f32 = 0.01;

#[must_use]
pub fn overlaps(rect: Rect, others: &[Rect]) -> bool {
    let shrunk = rect.inflate(-SLACK);
    others.iter().any(|other| shrunk.intersects(other))
}

#[must_use]
pub fn vacant(candidate: Rect, others: &[Rect], area: &Area) -> Rect {
    let contained = area.contain(candidate);
    if !overlaps(contained, others) {
        return contained;
    }
    let room = area.room();
    let (width, height) = (contained.width(), contained.height());
    let xs = stops(
        contained.min_x(),
        room.min_x(),
        room.max_x() - width,
        others
            .iter()
            .flat_map(|other| [other.max_x() + GAP, other.min_x() - GAP - width]),
    );
    let ys = stops(
        contained.min_y(),
        room.min_y(),
        room.max_y() - height,
        others
            .iter()
            .flat_map(|other| [other.max_y() + GAP, other.min_y() - GAP - height]),
    );
    let grown = room.inflate(SLACK);
    xs.iter()
        .flat_map(|x| ys.iter().map(move |y| Rect::new(*x, *y, width, height)))
        .filter(|rect| grown.contains_rect(rect) && !overlaps(*rect, others))
        .min_by(|a, b| distance(*a, contained).total_cmp(&distance(*b, contained)))
        .unwrap_or(contained)
}

#[must_use]
pub fn first_vacant(extent: Size, others: &[Rect], area: &Area) -> Option<Rect> {
    let room = area.room();
    let grown = room.inflate(SLACK);
    let mut xs = vec![room.min_x()];
    xs.extend(others.iter().map(|other| other.max_x() + GAP));
    let mut ys = vec![room.min_y()];
    ys.extend(others.iter().map(|other| other.max_y() + GAP));
    ys.iter()
        .flat_map(|y| {
            xs.iter()
                .map(move |x| Rect::new(*x, *y, extent.width, extent.height))
        })
        .filter(|rect| grown.contains_rect(rect) && !overlaps(*rect, others))
        .min_by(|a, b| {
            a.min_y()
                .total_cmp(&b.min_y())
                .then(a.min_x().total_cmp(&b.min_x()))
        })
}

fn stops(at: f32, first: f32, last: f32, beside: impl Iterator<Item = f32>) -> Vec<f32> {
    let mut stops = vec![at, first, last];
    stops.extend(beside);
    stops.retain(|stop| stop.is_finite());
    stops
}

fn distance(a: Rect, b: Rect) -> f32 {
    let dx = a.min_x() - b.min_x();
    let dy = a.min_y() - b.min_y();
    dx * dx + dy * dy
}
