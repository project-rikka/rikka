use rikka_types::{Color, Point, Rect};
use rikka_vector::{GradientShape, Spread, Stop, Transform};

use crate::parse::dom::{Dom, NodeId, Tag};
use crate::parse::values::{
    Length, Unit, aspect_ratio, length, reference, transform, unit_interval, view_box,
};
use crate::tree::style::{Axis, Context, Viewport};
use crate::tree::{Gradient, Units};

const MAX_LINKS: usize = 16;
const FOCUS_REACH: f32 = 0.999;

pub(crate) fn linked(dom: &Dom, node: NodeId, tags: &[Tag]) -> Vec<NodeId> {
    let mut chain = vec![node];
    let mut current = node;
    while chain.len() < MAX_LINKS {
        let next = dom
            .attribute(current, "href")
            .and_then(reference)
            .and_then(|id| dom.find(id, tags))
            .filter(|next| !chain.contains(next));
        match next {
            Some(next) => {
                chain.push(next);
                current = next;
            }
            None => break,
        }
    }
    chain
}

pub(crate) fn first<'d>(dom: &'d Dom, chain: &[NodeId], name: &str) -> Option<&'d str> {
    chain.iter().find_map(|node| dom.attribute(*node, name))
}

pub(crate) fn units(value: Option<&str>, default: Units) -> Units {
    match value {
        Some("userSpaceOnUse") => Units::UserSpace,
        Some("objectBoundingBox") => Units::BoundingBox,
        _ => default,
    }
}

pub(crate) fn measure(
    value: Option<&str>,
    default: Length,
    units: Units,
    axis: Axis,
    context: &Context<'_>,
) -> f32 {
    let length = value.and_then(length).unwrap_or(default);
    match (units, length.unit) {
        (Units::BoundingBox, Unit::Percent) => length.value / 100.0,
        (Units::BoundingBox, _) => length.value,
        (Units::UserSpace, _) => context.resolve(length, axis),
    }
}

pub(crate) enum Painted {
    Gradient(Gradient),
    Flat(Color),
    Nothing,
}

pub(crate) fn gradient(dom: &Dom, node: NodeId, viewport: Viewport) -> Painted {
    let tags = [Tag::LinearGradient, Tag::RadialGradient];
    let chain = linked(dom, node, &tags);
    let own = [node];
    let context = Context {
        dom,
        chain: &own,
        viewport,
    };
    let stops = chain
        .iter()
        .map(|link| stops(dom, *link, viewport))
        .find(|stops| !stops.is_empty())
        .unwrap_or_default();
    let Some(last) = stops.last().copied() else {
        return Painted::Nothing;
    };
    if stops.len() == 1 {
        return Painted::Flat(last.color);
    }
    let units = units(first(dom, &chain, "gradientUnits"), Units::BoundingBox);
    let Some(placed) = dom
        .attribute(node, "gradientTransform")
        .map_or(Some(Transform::IDENTITY), transform)
    else {
        return Painted::Nothing;
    };
    let spread = match first(dom, &chain, "spreadMethod") {
        Some("reflect") => Spread::Reflect,
        Some("repeat") => Spread::Repeat,
        _ => Spread::Pad,
    };
    let at = |name: &str, default: Length, axis: Axis| {
        measure(first(dom, &chain, name), default, units, axis, &context)
    };
    let shape = match dom.element(node).tag {
        Tag::LinearGradient => {
            let start = Point::new(
                at("x1", Length::ZERO, Axis::Horizontal),
                at("y1", Length::ZERO, Axis::Vertical),
            );
            let end = Point::new(
                at("x2", Length::percent(100.0), Axis::Horizontal),
                at("y2", Length::ZERO, Axis::Vertical),
            );
            if start == end {
                return Painted::Flat(last.color);
            }
            GradientShape::Linear { start, end }
        }
        _ => {
            let half = Length::percent(50.0);
            let center = Point::new(
                at("cx", half, Axis::Horizontal),
                at("cy", half, Axis::Vertical),
            );
            let radius = at("r", half, Axis::Diagonal);
            if radius <= 0.0 || radius.is_nan() {
                return Painted::Flat(last.color);
            }
            let focus_axis = |name: &str, axis: Axis, fallback: f32| match first(dom, &chain, name)
            {
                Some(_) => at(name, Length::ZERO, axis),
                None => fallback,
            };
            let mut focus = Point::new(
                focus_axis("fx", Axis::Horizontal, center.x),
                focus_axis("fy", Axis::Vertical, center.y),
            );
            let focus_radius = at("fr", Length::ZERO, Axis::Diagonal).max(0.0);
            let away = focus.distance(center);
            let reach = radius * FOCUS_REACH;
            if away > reach {
                focus = center + (focus - center) * (reach / away);
            }
            GradientShape::Radial {
                start_center: focus,
                start_radius: focus_radius.min(reach),
                end_center: center,
                end_radius: radius,
            }
        }
    };
    Painted::Gradient(Gradient {
        units,
        transform: placed,
        spread,
        stops,
        shape,
    })
}

fn ancestry(dom: &Dom, node: NodeId) -> Vec<NodeId> {
    let mut path = vec![node];
    let mut current = node;
    while let Some(parent) = dom.element(current).parent {
        path.push(parent);
        current = parent;
    }
    path.reverse();
    path
}

fn stops(dom: &Dom, gradient: NodeId, viewport: Viewport) -> Vec<Stop> {
    let mut found: Vec<Stop> = Vec::new();
    let mut chain = ancestry(dom, gradient);
    for &child in &dom.element(gradient).children {
        if dom.element(child).tag != Tag::Stop {
            continue;
        }
        chain.push(child);
        let context = Context {
            dom,
            chain: &chain,
            viewport,
        };
        let offset = context
            .attribute("offset")
            .and_then(unit_interval)
            .unwrap_or(0.0)
            .max(found.last().map_or(0.0, |previous| previous.offset));
        let mut color = context
            .color("stop-color")
            .unwrap_or(Color::rgba(0.0, 0.0, 0.0, 1.0));
        color.a *= context.unit("stop-opacity").unwrap_or(1.0);
        found.push(Stop { offset, color });
        chain.pop();
    }
    if let (Some(first), Some(last)) = (found.first().copied(), found.last().copied()) {
        if first.offset > 0.0 {
            found.insert(
                0,
                Stop {
                    offset: 0.0,
                    color: first.color,
                },
            );
        }
        if last.offset < 1.0 {
            found.push(Stop {
                offset: 1.0,
                color: last.color,
            });
        }
    }
    found
}

pub(crate) struct Tile {
    pub(crate) units: Units,
    pub(crate) content_units: Units,
    pub(crate) transform: Transform,
    pub(crate) rect: Rect,
    pub(crate) view: Option<(
        crate::parse::values::ViewBox,
        crate::parse::values::AspectRatio,
    )>,
    pub(crate) content: NodeId,
}

pub(crate) fn tile(dom: &Dom, node: NodeId, viewport: Viewport) -> Option<Tile> {
    let chain = linked(dom, node, &[Tag::Pattern]);
    let own = [node];
    let context = Context {
        dom,
        chain: &own,
        viewport,
    };
    let units = units(first(dom, &chain, "patternUnits"), Units::BoundingBox);
    let content_units = units_of_content(first(dom, &chain, "patternContentUnits"));
    let at = |name: &str, axis: Axis| {
        measure(
            first(dom, &chain, name),
            Length::ZERO,
            units,
            axis,
            &context,
        )
    };
    let rect = Rect::new(
        at("x", Axis::Horizontal),
        at("y", Axis::Vertical),
        at("width", Axis::Horizontal),
        at("height", Axis::Vertical),
    );
    if !(rect.width() > 0.0 && rect.height() > 0.0) {
        return None;
    }
    let content = chain
        .iter()
        .copied()
        .find(|link| !dom.element(*link).children.is_empty())?;
    Some(Tile {
        units,
        content_units,
        transform: dom
            .attribute(node, "patternTransform")
            .map_or(Some(Transform::IDENTITY), transform)?,
        rect,
        view: first(dom, &chain, "viewBox")
            .and_then(view_box)
            .map(|view| {
                let aspect = first(dom, &chain, "preserveAspectRatio")
                    .map(aspect_ratio)
                    .unwrap_or_default();
                (view, aspect)
            }),
        content,
    })
}

fn units_of_content(value: Option<&str>) -> Units {
    units(value, Units::UserSpace)
}
