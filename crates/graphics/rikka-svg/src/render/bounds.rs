use rikka_types::{Point, Rect};
use rikka_vector::{Join, Transform};

use crate::tree::{Group, Node, Units};

pub(crate) fn placed(rect: Rect, transform: &Transform) -> Rect {
    let corners = rect.corners().map(|corner| transform.apply(corner));
    let low = corners
        .iter()
        .fold(corners[0], |low, corner| low.min(*corner));
    let high = corners
        .iter()
        .fold(corners[0], |high, corner| high.max(*corner));
    Rect::new(low.x, low.y, high.x - low.x, high.y - low.y)
}

pub(crate) fn unit_box(bounds: Rect) -> Transform {
    Transform::scale(bounds.width(), bounds.height())
        .then(Transform::translate(bounds.min_x(), bounds.min_y()))
}

pub(crate) fn within(rect: Rect, units: Units, bounds: Option<Rect>) -> Option<Rect> {
    match units {
        Units::UserSpace => Some(rect),
        Units::BoundingBox => {
            let bounds = bounds?;
            Some(Rect::new(
                bounds.min_x() + rect.min_x() * bounds.width(),
                bounds.min_y() + rect.min_y() * bounds.height(),
                rect.width() * bounds.width(),
                rect.height() * bounds.height(),
            ))
        }
    }
}

fn union(one: Option<Rect>, other: Option<Rect>) -> Option<Rect> {
    match (one, other) {
        (Some(one), Some(other)) => Some(one.union(&other)),
        (one, other) => one.or(other),
    }
}

pub(crate) fn content_bounds(group: &Group, transform: &Transform) -> Option<Rect> {
    group.children.iter().fold(None, |total, child| {
        union(total, node_bounds(child, transform, false))
    })
}

pub(crate) fn node_bounds(node: &Node, transform: &Transform, inked: bool) -> Option<Rect> {
    match node {
        Node::Shape(shape) => {
            let placement = shape.transform.then(*transform);
            match shape.stroke.as_ref().filter(|_| inked) {
                Some(stroke) => {
                    let reach = stroke.width
                        * 0.5
                        * match stroke.join {
                            Join::Miter => stroke.miter_limit.max(std::f32::consts::SQRT_2),
                            _ => std::f32::consts::SQRT_2,
                        };
                    let bounds = shape.path.bounds()?.inflate(reach);
                    Some(placed(bounds, &placement))
                }
                None => shape.path.transformed(&placement).bounds(),
            }
        }
        Node::Picture(picture) => Some(placed(picture.frame, &picture.transform.then(*transform))),
        Node::Glyph(glyph) => Some(placed(glyph.extent, &glyph.transform.then(*transform))),
        Node::Group(group) => {
            let placement = group.transform.then(*transform);
            if inked && let Some(filter) = group.filters.first() {
                let object = content_bounds(group, &Transform::IDENTITY);
                return within(filter.rect, filter.units, object)
                    .map(|region| placed(region, &placement));
            }
            group.children.iter().fold(None, |total, child| {
                union(total, node_bounds(child, &placement, inked))
            })
        }
    }
}

pub(crate) fn scales(transform: &Transform) -> Point {
    Point::new(
        transform.xx.hypot(transform.yx),
        transform.xy.hypot(transform.yy),
    )
}
