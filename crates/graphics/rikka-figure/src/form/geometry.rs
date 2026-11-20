use rikka_image_edit::{FillRule, Path, PathMark, StrokeStyle};
use rikka_types::{Point, Rect};

use super::Form;
use crate::affine::Affine;
use crate::frame::{Frame, enclosing};
use crate::look::{Look, Pen};
use crate::shape::Shape;
use crate::text::{Lettering, Typeset, decorated_bounds};

const HEAD_WIDTH: f32 = 3.0;
const MIN_HEAD_WIDTH: f32 = 12.0;
const MIN_SPAN: f32 = 3.0;
const MIN_CORNERS: usize = 3;

pub(super) fn head_width(width: f32) -> f32 {
    (width * HEAD_WIDTH).max(MIN_HEAD_WIDTH)
}

fn points(path: &Path) -> impl Iterator<Item = Point> + '_ {
    path.subpaths()
        .iter()
        .flat_map(|subpath| subpath.points().iter().copied())
}

pub(super) fn frame(form: &Form) -> Option<Frame> {
    match form {
        Form::Line { .. } | Form::Arrow { .. } => None,
        Form::Shape { frame, .. } => Some(*frame),
        Form::Path(figure) => Some(Frame::from_rect(enclosing(points(figure.path())))),
        Form::Text(lettering) => Some(Frame::from_rect(Typeset::new(lettering).layout())),
    }
}

pub(super) fn bounds(form: &Form) -> Rect {
    match form {
        Form::Line { from, to, pen } => enclosing([*from, *to]).inflate(pen.width / 2.0),
        Form::Arrow { from, to, pen } => {
            enclosing([*from, *to]).inflate(pen.width.max(head_width(pen.width)) / 2.0)
        }
        Form::Shape { shape, frame, look } => {
            enclosing(points(&shape.path(frame))).inflate(look.reach())
        }
        Form::Path(figure) => {
            let reach = figure
                .stroke()
                .map_or(0.0, |(_, stroke)| stroke.width / 2.0);
            enclosing(points(figure.path())).inflate(reach)
        }
        Form::Text(lettering) => decorated_bounds(lettering, &Typeset::new(lettering)),
    }
}

pub(super) fn hits(form: &Form, at: Point, reach: f32) -> bool {
    match form {
        Form::Line { from, to, pen } => gap(*from, *to, at) <= reach + pen.width / 2.0,
        Form::Arrow { from, to, pen } => {
            gap(*from, *to, at) <= reach + pen.width / 2.0
                || at.distance(*to) <= reach + head_width(pen.width) / 2.0
        }
        Form::Shape { shape, frame, look } => {
            let path = shape.path(frame);
            let filled = look.fill.map(|_| FillRule::NonZero);
            path_hits(&path, filled, at, reach + look.reach())
        }
        Form::Path(figure) => {
            let edge = reach
                + figure
                    .stroke()
                    .map_or(0.0, |(_, stroke)| stroke.width / 2.0);
            path_hits(figure.path(), figure.fill().map(|(_, rule)| rule), at, edge)
        }
        Form::Text(lettering) => {
            let typeset = Typeset::new(lettering);
            let grown = lettering.outline_reach() + reach;
            let band = lettering.band.map_or(0.0, |band| band.padding);
            typeset
                .layout()
                .inflate(grown.max(band + reach))
                .contains(at)
        }
    }
}

fn path_hits(path: &Path, filled: Option<FillRule>, at: Point, edge: f32) -> bool {
    let subpaths = path.subpaths();
    if let Some(rule) = filled {
        let mut winding = 0i32;
        let mut crossings = 0u32;
        for subpath in subpaths {
            let points = subpath.points();
            for (from, to) in points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
            {
                if let Some(up) = crosses(*from, *to, at) {
                    crossings += 1;
                    winding += if up { 1 } else { -1 };
                }
            }
        }
        let inside = match rule {
            FillRule::NonZero => winding != 0,
            FillRule::EvenOdd => crossings % 2 == 1,
        };
        if inside {
            return true;
        }
    }
    subpaths.iter().any(|subpath| {
        let points = subpath.points();
        match subpath.is_closed() {
            true => near_loop(points, at, edge),
            false => match points {
                [only] => only.distance(at) <= edge,
                _ => points
                    .windows(2)
                    .any(|pair| gap(pair[0], pair[1], at) <= edge),
            },
        }
    })
}

fn crosses(from: Point, to: Point, at: Point) -> Option<bool> {
    let up = from.y <= at.y && to.y > at.y;
    let down = from.y > at.y && to.y <= at.y;
    if !up && !down {
        return None;
    }
    let along = (at.y - from.y) / (to.y - from.y);
    let x = from.x + along * (to.x - from.x);
    (x > at.x).then_some(up)
}

fn near_loop(points: &[Point], at: Point, edge: f32) -> bool {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .any(|(from, to)| gap(*from, *to, at) <= edge)
}

pub(crate) fn gap(from: Point, to: Point, at: Point) -> f32 {
    let along = to - from;
    let length = along.x * along.x + along.y * along.y;
    if length <= f32::EPSILON {
        return from.distance(at);
    }
    let reached =
        (((at.x - from.x) * along.x + (at.y - from.y) * along.y) / length).clamp(0.0, 1.0);
    (from + along * reached).distance(at)
}

pub(super) fn mapped(form: &Form, map: Affine, strokes: bool) -> Form {
    let thick = |width: f32| match strokes {
        true => width * map.stretch(),
        false => width,
    };
    let pen = |pen: &Pen| Pen {
        width: thick(pen.width),
        ..*pen
    };
    match form {
        Form::Line {
            from,
            to,
            pen: original,
        } => Form::Line {
            from: map.apply(*from),
            to: map.apply(*to),
            pen: pen(original),
        },
        Form::Arrow {
            from,
            to,
            pen: original,
        } => Form::Arrow {
            from: map.apply(*from),
            to: map.apply(*to),
            pen: pen(original),
        },
        Form::Shape { shape, frame, look } => Form::Shape {
            shape: match shape {
                Shape::Rectangle { radius } => Shape::Rectangle {
                    radius: thick(*radius),
                },
                other => *other,
            },
            frame: frame.mapped(map),
            look: Look {
                fill: look.fill,
                outline: look.outline.as_ref().map(pen),
            },
        },
        Form::Path(figure) => Form::Path(mapped_path(figure, map, thick)),
        Form::Text(lettering) => Form::Text(mapped_text(lettering, map, strokes)),
    }
}

fn mapped_path(figure: &PathMark, map: Affine, thick: impl Fn(f32) -> f32) -> PathMark {
    let mut builder = Path::builder();
    for subpath in figure.path().subpaths() {
        for (at, point) in subpath.points().iter().enumerate() {
            let placed = map.apply(*point);
            builder = match at {
                0 => builder.move_to(placed),
                _ => builder.line_to(placed),
            };
        }
        if subpath.is_closed() {
            builder = builder.close();
        }
    }
    let mut mapped = PathMark::new(builder.build()).antialiased(figure.is_antialiased());
    if let Some((color, rule)) = figure.fill() {
        mapped = mapped.filled(color, rule);
    }
    if let Some((color, stroke)) = figure.stroke() {
        mapped = mapped.stroked(
            color,
            StrokeStyle {
                width: thick(stroke.width),
                ..stroke
            },
        );
    }
    mapped
}

fn mapped_text(lettering: &Lettering, map: Affine, strokes: bool) -> Lettering {
    if map.is_translation() {
        return Lettering {
            origin: (
                lettering.origin.0 + map.tx.round() as i32,
                lettering.origin.1 + map.ty.round() as i32,
            ),
            ..lettering.clone()
        };
    }
    let center = map.apply(Typeset::new(lettering).layout().center());
    let mut mapped = match strokes {
        true => lettering.scaled(map.stretch()),
        false => lettering.clone(),
    };
    let laid = Typeset::new(&Lettering {
        origin: (0, 0),
        ..mapped.clone()
    })
    .layout();
    mapped.origin = (
        (center.x - laid.width() / 2.0).round() as i32,
        (center.y - laid.height() / 2.0).round() as i32,
    );
    mapped
}

pub(super) fn is_worth_keeping(form: &Form) -> bool {
    match form {
        Form::Line { from, to, .. } | Form::Arrow { from, to, .. } => {
            (to.x - from.x).abs().max((to.y - from.y).abs()) >= MIN_SPAN
        }
        Form::Shape { frame, .. } => frame.width.max(frame.height) >= MIN_SPAN,
        Form::Path(figure) => {
            let corners: usize = figure
                .path()
                .subpaths()
                .iter()
                .map(|subpath| subpath.points().len())
                .sum();
            let extent = enclosing(points(figure.path()));
            corners >= MIN_CORNERS && extent.width().max(extent.height()) >= MIN_SPAN
        }
        Form::Text(lettering) => !lettering.text.trim().is_empty(),
    }
}
