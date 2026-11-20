use rikka_types::{Point, Rect};
use rikka_vector::Path;

use crate::parse::dom::Tag;
use crate::parse::path_data::path_data;
use crate::parse::values::numbers;
use crate::tree::style::{Axis, Context};

pub(crate) fn outline(tag: Tag, context: &Context<'_>) -> Option<Path> {
    let x = |name: &str| context.coordinate(name, Axis::Horizontal);
    let y = |name: &str| context.coordinate(name, Axis::Vertical);
    let mut path = Path::new();
    match tag {
        Tag::Path => path = path_data(context.attribute("d")?),
        Tag::Rect => {
            let width = x("width").filter(|width| *width > 0.0)?;
            let height = y("height").filter(|height| *height > 0.0)?;
            let rounding = |name: &str, axis| {
                context
                    .attribute(name)
                    .filter(|value| *value != "auto")
                    .and_then(|_| context.coordinate(name, axis))
                    .filter(|radius| *radius >= 0.0)
            };
            let (rx, ry) = match (
                rounding("rx", Axis::Horizontal),
                rounding("ry", Axis::Vertical),
            ) {
                (Some(rx), Some(ry)) => (rx, ry),
                (Some(radius), None) | (None, Some(radius)) => (radius, radius),
                (None, None) => (0.0, 0.0),
            };
            let frame = Rect::new(x("x").unwrap_or(0.0), y("y").unwrap_or(0.0), width, height);
            path.add_rounded_rect(frame, Point::new(rx, ry));
        }
        Tag::Circle => {
            let radius = context
                .coordinate("r", Axis::Diagonal)
                .filter(|radius| *radius > 0.0)?;
            let center = Point::new(x("cx").unwrap_or(0.0), y("cy").unwrap_or(0.0));
            path.add_ellipse(center, Point::new(radius, radius));
        }
        Tag::Ellipse => {
            let radii = match (x("rx"), y("ry")) {
                (Some(rx), Some(ry)) => (rx, ry),
                (Some(radius), None) | (None, Some(radius)) => (radius, radius),
                (None, None) => return None,
            };
            if radii.0 <= 0.0 || radii.1 <= 0.0 {
                return None;
            }
            let center = Point::new(x("cx").unwrap_or(0.0), y("cy").unwrap_or(0.0));
            path.add_ellipse(center, Point::new(radii.0, radii.1));
        }
        Tag::Line => {
            path.move_to(Point::new(x("x1").unwrap_or(0.0), y("y1").unwrap_or(0.0)));
            path.line_to(Point::new(x("x2").unwrap_or(0.0), y("y2").unwrap_or(0.0)));
        }
        Tag::Polyline | Tag::Polygon => {
            let values = numbers(context.attribute("points")?);
            let points = values.as_chunks::<2>().0;
            if points.len() < 2 {
                return None;
            }
            for (index, &[px, py]) in points.iter().enumerate() {
                match index {
                    0 => path.move_to(Point::new(px, py)),
                    _ => path.line_to(Point::new(px, py)),
                }
            }
            if tag == Tag::Polygon {
                path.close();
            }
        }
        _ => return None,
    }
    (!path.is_empty() && !path.is_truncated()).then_some(path)
}
