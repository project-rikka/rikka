use cut_project::{Area, Art, Format, Graphic, Placement, Region};
use rikka_figure::{Lettering, Look, Pen, Shape, ShapeKind, TextAlign};
use rikka_text::NOTO_SANS_JP;
use rikka_types::{Color, Point};

use crate::tool::Tool;

const SHAPE_SHARE: f32 = 0.3;
const RANGE_SHARE: f32 = 0.4;
const LINE_SHARE: f32 = 0.25;
const TEXT_SHARE: f32 = 0.075;
const PEN_SHARE: f32 = 0.0075;
const EDGE_SHARE: f32 = 0.09;
const SHAPE_FILL: Color = Color::rgb(0.24, 0.52, 0.96);
const ARROW_INK: Color = Color::rgb(0.95, 0.27, 0.23);

fn short_side(format: Format) -> f32 {
    format.width.min(format.height) as f32
}

fn placed_at(format: Format, at: Point) -> Placement {
    Placement {
        x: at.x - format.width as f32 / 2.0,
        y: at.y - format.height as f32 / 2.0,
        ..Placement::default()
    }
}

fn sized(kind: ShapeKind, side: f32) -> (f32, f32) {
    match kind {
        ShapeKind::Rectangle | ShapeKind::RoundedRectangle => (side * 16.0 / 9.0, side),
        ShapeKind::Ellipse | ShapeKind::Triangle | ShapeKind::Polygon | ShapeKind::Star => {
            (side, side)
        }
    }
}

fn spanned(format: Format, from: Point, to: Point) -> (f32, f32, Placement) {
    let center = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    (
        (to.x - from.x).abs(),
        (to.y - from.y).abs(),
        placed_at(format, center),
    )
}

fn shape_art(kind: ShapeKind, width: f32, height: f32) -> Art {
    Art::Shape {
        shape: Shape::from(kind),
        width,
        height,
        look: Look {
            fill: Some(SHAPE_FILL),
            outline: None,
        },
    }
}

fn line_art(format: Format, reach: Point, arrow: bool) -> Art {
    Art::Line {
        reach,
        pen: Pen::new(
            match arrow {
                true => ARROW_INK,
                false => Color::WHITE,
            },
            (short_side(format) * PEN_SHARE).round().max(1.0),
        ),
        arrow,
    }
}

pub(crate) fn edge_width(size: f32) -> f32 {
    (size * EDGE_SHARE).round().max(1.0)
}

pub(crate) fn lettering(format: Format, text: &str) -> Lettering {
    let size = (format.height as f32 * TEXT_SHARE).round().max(8.0);
    Lettering {
        text: text.to_owned(),
        font: NOTO_SANS_JP.to_owned(),
        size,
        weight: Lettering::BOLD,
        align: TextAlign::Center,
        color: Color::WHITE,
        outlines: vec![Pen::new(Color::BLACK, edge_width(size))],
        ..Lettering::default()
    }
}

pub(crate) fn new_graphic(tool: Tool, format: Format, text: &str) -> Option<Graphic> {
    let side = short_side(format) * SHAPE_SHARE;
    let art = match tool {
        Tool::Select | Tool::Anchor | Tool::Range(_) => return None,
        Tool::Text => Art::text(lettering(format, text)),
        Tool::Shape(kind) => {
            let (width, height) = sized(kind, side);
            shape_art(kind, width, height)
        }
        Tool::Line | Tool::Arrow => line_art(
            format,
            Point::new(format.width as f32 * LINE_SHARE, 0.0),
            tool == Tool::Arrow,
        ),
    };
    Some(Graphic::new(art))
}

pub(crate) fn drawn(tool: Tool, format: Format, from: Point, to: Point) -> Option<Graphic> {
    let (width, height, placement) = spanned(format, from, to);
    let art = match tool {
        Tool::Shape(kind) => shape_art(kind, width, height),
        Tool::Line | Tool::Arrow => line_art(format, to - from, tool == Tool::Arrow),
        Tool::Select | Tool::Anchor | Tool::Text | Tool::Range(_) => return None,
    };
    Some(Graphic::new(art).with_placement(placement))
}

pub(crate) fn new_text(format: Format, at: Point, text: &str) -> Graphic {
    Graphic::new(Art::text(lettering(format, text))).with_placement(placed_at(format, at))
}

pub(crate) fn area(kind: ShapeKind, format: Format) -> Area {
    let (width, height) = sized(kind, short_side(format) * RANGE_SHARE);
    Area::Shape {
        shape: Shape::from(kind),
        width,
        height,
    }
}

pub(crate) fn range(shape: Option<ShapeKind>, format: Format) -> Region {
    match shape {
        Some(kind) => Region {
            area: area(kind, format),
            ..Region::whole()
        },
        None => Region::whole(),
    }
}

pub(crate) fn ranged(kind: ShapeKind, format: Format, from: Point, to: Point) -> Region {
    let (width, height, placement) = spanned(format, from, to);
    Region::shaped(Shape::from(kind), width, height).with_placement(placement)
}
