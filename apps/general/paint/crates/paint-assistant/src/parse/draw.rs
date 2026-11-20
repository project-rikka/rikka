use rikka_image_edit::{
    Anchor, Cap, FillRule, GradientInk, Join, PathMark, StrokeStyle, Tool as MarkTool,
};
use rikka_types::Point;
use serde::Deserialize;
use serde_json::{Map, Value as Json};

use super::item::{Item, changed, layer_ref, numbered, sized};
use super::read;
use crate::ops::Parsed;
use crate::order::{
    BrushOrder, Change, DrawKind, DrawOrder, Filling, FloodOrder, GradientOrder, MarkOrder, Order,
    PathOrder, Placing, TextOrder,
};
use crate::svg;
use crate::vocabulary::{
    ALIGNS, ANCHORS, CAPS, FILL_RULES, FILLINGS, FLOODS, GRADIENTS, ITEMS, JOINS, MARKS, SAMPLES,
};

const MAX_MARK_POINTS: usize = 20_000;
const WIDTHS: (f32, f32) = (0.5, 500.0);
const SIZES: (f32, f32) = (1.0, 500.0);
const PATH_WIDTH: f32 = 4.0;
const TEXT_SIZES: (f32, f32) = (1.0, 2000.0);
const WEIGHTS: (f32, f32) = (1.0, 1000.0);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DrawArgs {
    #[serde(default)]
    document: Option<String>,
    name: String,
    #[serde(default)]
    layer: Option<Json>,
    items: Vec<Map<String, Json>>,
}

pub(crate) fn draw(input: Json) -> Parsed<Order> {
    let args: DrawArgs = read(input)?;
    let layer = args.layer.as_ref().map(layer_ref).transpose()?;
    let items = numbered(args.items, "item", drawn)?;
    changed(
        args.document,
        args.name,
        Change::Draw(DrawOrder { layer, items }),
    )
}

pub(super) fn drawn(fields: Map<String, Json>) -> Parsed<DrawKind> {
    let mut item = Item::read(fields, "tool", &ITEMS, "tool")?;
    Ok(match item.kind() {
        "brush" | "eraser" => DrawKind::Brush(brush(item)?),
        "path" => DrawKind::Path(path(item)?),
        "gradient" => DrawKind::Gradient(gradient(item)?),
        "text" => DrawKind::Text(text(item)?),
        "bucket" => DrawKind::Bucket(flood(&mut item)?),
        "fill" => DrawKind::Fill(item.color("color")?),
        "erase" => DrawKind::Erase(item.color("color")?),
        _ => DrawKind::Mark(mark(item)?),
    })
}

pub(super) fn point([x, y]: [f32; 2]) -> Parsed<Point> {
    match x.is_finite() && y.is_finite() {
        true => Ok(Point::new(x, y)),
        false => Err("a point is not a number".to_owned()),
    }
}

fn trail(points: Vec<[f32; 2]>) -> Parsed<Vec<Point>> {
    if points.is_empty() || points.len() > MAX_MARK_POINTS {
        return Err(format!(
            "give 1–{MAX_MARK_POINTS} points, not {}",
            points.len()
        ));
    }
    points.into_iter().map(point).collect()
}

fn mark(mut item: Item) -> Parsed<MarkOrder> {
    let tool = MARKS
        .iter()
        .find(|(name, _)| *name == item.kind())
        .map(|(_, tool)| *tool)
        .ok_or_else(|| format!("{} is no mark", item.kind()))?;
    let strokes = match tool {
        MarkTool::Rectangle | MarkTool::Ellipse => {
            let [x, y, width, height] = sized(item.need("rect", "[x, y, width, height]")?, "rect")?;
            vec![vec![Point::new(x, y), Point::new(x + width, y + height)]]
        }
        MarkTool::Line | MarkTool::Arrow => {
            let points: Vec<[f32; 2]> = item.need("points", "[[x1, y1], [x2, y2]]")?;
            if points.len() != 2 {
                return Err(format!(
                    "give exactly 2 points, from and to, not {}",
                    points.len()
                ));
            }
            vec![trail(points)?]
        }
        MarkTool::Pen | MarkTool::Highlighter => {
            vec![trail(item.need("points", "[[x, y], …]")?)?]
        }
    };
    let angle = match item.take::<f32>("angle")? {
        Some(angle) if !angle.is_finite() => {
            return Err(format!(
                "`angle` is {angle}; give degrees, positive clockwise"
            ));
        }
        angle => angle,
    };
    let color = item.color("color")?;
    let width = item.number_in_range("width", WIDTHS.0, WIDTHS.1)?;
    let inside = item.color("fill")?;
    let filling = match item.take::<String>("filling")?.as_deref() {
        None => None,
        Some("outline") => Some(Filling::Outline),
        Some("fill") => Some(Filling::Fill),
        Some("both") => Some(Filling::Both(inside)),
        Some(other) => {
            return Err(format!(
                "filling is \"{other}\"; use {}",
                FILLINGS.join(", ")
            ));
        }
    };
    if inside.is_some() && !matches!(filling, Some(Filling::Both(_))) {
        return Err("`fill` colours the inside when `filling` is \"both\"".to_owned());
    }
    Ok(MarkOrder {
        tool,
        strokes,
        color,
        width,
        filling,
        angle,
    })
}

fn strokes(item: &mut Item) -> Parsed<Vec<Vec<Point>>> {
    match (
        item.take::<Vec<[f32; 2]>>("points")?,
        item.take::<String>("d")?,
    ) {
        (Some(points), None) => Ok(vec![trail(points)?]),
        (None, Some(d)) => Ok(svg::outlines(&svg::path(&d)?)
            .into_iter()
            .map(|outline| outline.into_iter().map(|(x, y)| Point::new(x, y)).collect())
            .collect()),
        _ => Err("give exactly one of `points` [[x, y], …] or `d` (SVG path data)".to_owned()),
    }
}

fn brush(mut item: Item) -> Parsed<BrushOrder> {
    Ok(BrushOrder {
        erases: item.kind() == "eraser",
        strokes: strokes(&mut item)?,
        size: item.number_in_range("size", SIZES.0, SIZES.1)?,
        hardness: item.percent("hardness")?,
        opacity: item.percent("opacity")?,
        flow: item.percent("flow")?,
        antialias: item.take("antialias")?,
        color: item.color("color")?,
    })
}

fn path(mut item: Item) -> Parsed<PathOrder> {
    let d: String = item.need("d", "SVG path data")?;
    let mut figure = PathMark::new(svg::path(&d)?);
    let fill = item.color("fill")?;
    let stroke = item.color("stroke")?;
    let rule: FillRule = item.named("fill_rule", &FILL_RULES)?.unwrap_or_default();
    let width = item.number_in_range("width", WIDTHS.0, WIDTHS.1)?;
    let join: Join = item.named("join", &JOINS)?.unwrap_or_default();
    let cap: Cap = item.named("cap", &CAPS)?.unwrap_or_default();
    if let Some(fill) = fill {
        figure = figure.filled(fill, rule);
    }
    match stroke {
        Some(stroke) => {
            let style = StrokeStyle::new(width.unwrap_or(PATH_WIDTH))
                .join(join)
                .cap(cap);
            figure = figure.stroked(stroke, style);
        }
        None if width.is_some() => {
            return Err("`width` is the outline's; give `stroke` its colour".to_owned());
        }
        None => {}
    }
    if fill.is_none() && stroke.is_none() {
        return Err("a path needs `fill`, `stroke` or both, as colours".to_owned());
    }
    Ok(PathOrder {
        figure,
        antialias: item.take("antialias")?,
    })
}

fn gradient(mut item: Item) -> Parsed<GradientOrder> {
    let shape = item.named("shape", &GRADIENTS)?.unwrap_or_default();
    let from = point(item.need("from", "[x, y] where the gradient starts")?)?;
    let to = point(item.need("to", "[x, y] where it ends")?)?;
    let colors: Option<[Json; 2]> = item.take("colors")?;
    let fade: Option<[f32; 2]> = item.take("fade")?;
    let ink = match (colors, fade) {
        (Some([start, end]), None) => Some(GradientInk::Colors {
            from: crate::vocabulary::color(&start)?,
            to: crate::vocabulary::color(&end)?,
        }),
        (None, Some([start, end])) => {
            if !(0.0..=100.0).contains(&start) || !(0.0..=100.0).contains(&end) {
                return Err(format!("fade is [{start}, {end}]; use opacities 0–100"));
            }
            Some(GradientInk::Fade {
                from: start / 100.0,
                to: end / 100.0,
            })
        }
        (None, None) => None,
        (Some(_), Some(_)) => {
            return Err("give `colors` or `fade`, not both".to_owned());
        }
    };
    if from == to {
        return Err("`from` and `to` are the same point; a gradient needs a length".to_owned());
    }
    Ok(GradientOrder {
        shape,
        from,
        to,
        ink,
    })
}

fn text(mut item: Item) -> Parsed<TextOrder> {
    let text: String = item.need("text", "the words to set")?;
    if text.trim().is_empty() {
        return Err("`text` is empty".to_owned());
    }
    let anchor: Option<Anchor> = item.named("anchor", &ANCHORS)?;
    let at: Option<[f32; 2]> = item.take("at")?;
    let inside: Option<[f32; 4]> = item.take("inside")?;
    let place = match (at, inside) {
        (Some(at), None) => Placing::At {
            point: (point(at)?.x, point(at)?.y),
            anchor: anchor.unwrap_or(Anchor::TopLeft),
        },
        (None, Some(rect)) => Placing::Inside {
            rect: sized(rect, "inside")?,
            anchor: anchor.unwrap_or(Anchor::Center),
        },
        _ => {
            return Err(
                "place the text with exactly one of `at` [x, y] or `inside` [x, y, width, height]"
                    .to_owned(),
            );
        }
    };
    let wrap: Option<u32> = item.take("wrap")?;
    if wrap == Some(0) {
        return Err("`wrap` is the box width in px; give at least 1".to_owned());
    }
    Ok(TextOrder {
        text,
        font: item.take("font")?,
        size: item.number_in_range("size", TEXT_SIZES.0, TEXT_SIZES.1)?,
        weight: item
            .number_in_range("weight", WEIGHTS.0, WEIGHTS.1)?
            .map(|weight| weight.round() as u16),
        italic: item.take("italic")?,
        align: item.named("align", &ALIGNS)?,
        wrap,
        antialias: item.take("antialias")?,
        color: item.color("color")?,
        place,
    })
}

pub(super) fn flood(item: &mut Item) -> Parsed<FloodOrder> {
    let [x, y] = item.need::<[f32; 2]>("at", "the [x, y] pixel to start from")?;
    if !(x.is_finite() && y.is_finite()) || x < 0.0 || y < 0.0 {
        return Err(format!(
            "`at` is [{x}, {y}]; give a pixel inside the picture"
        ));
    }
    Ok(FloodOrder {
        at: (x, y),
        tolerance: item.percent("tolerance")?,
        mode: item.named("mode", &FLOODS)?,
        sample: item.named("sample", &SAMPLES)?,
        antialias: item.take("antialias")?,
        color: item.color("color")?,
    })
}
