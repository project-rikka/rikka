mod geometry;
mod marking;

use rikka_image_edit::PathMark;
use rikka_types::{Point, Rect};

use crate::affine::Affine;
use crate::frame::Frame;
use crate::look::{Look, Pen};
use crate::shape::{Shape, ShapeKind};
use crate::text::Lettering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormKind {
    Line,
    Arrow,
    Shape(ShapeKind),
    Path,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Form {
    Line {
        from: Point,
        to: Point,
        pen: Pen,
    },
    Arrow {
        from: Point,
        to: Point,
        pen: Pen,
    },
    Shape {
        shape: Shape,
        frame: Frame,
        look: Look,
    },
    Path(PathMark),
    Text(Lettering),
}

impl Form {
    #[must_use]
    pub fn spanned(kind: FormKind, from: Point, to: Point, look: Look) -> Option<Form> {
        let pen = look.outline.unwrap_or(Pen::new(look.ink(), 1.0));
        Some(match kind {
            FormKind::Line => Form::Line { from, to, pen },
            FormKind::Arrow => Form::Arrow { from, to, pen },
            FormKind::Shape(kind) => Form::Shape {
                shape: Shape::from(kind),
                frame: Frame::from_corners(from, to),
                look,
            },
            FormKind::Path | FormKind::Text => return None,
        })
    }

    #[must_use]
    pub fn kind(&self) -> FormKind {
        match self {
            Form::Line { .. } => FormKind::Line,
            Form::Arrow { .. } => FormKind::Arrow,
            Form::Shape { shape, .. } => FormKind::Shape(shape.kind()),
            Form::Path(_) => FormKind::Path,
            Form::Text(_) => FormKind::Text,
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        geometry::bounds(self)
    }

    #[must_use]
    pub fn frame(&self) -> Option<Frame> {
        geometry::frame(self)
    }

    #[must_use]
    pub fn extent(&self) -> Rect {
        match (self, self.frame()) {
            (Form::Line { from, to, .. } | Form::Arrow { from, to, .. }, _) => {
                crate::frame::enclosing([*from, *to])
            }
            (_, Some(frame)) => frame.bounds(),
            (_, None) => self.bounds(),
        }
    }

    #[must_use]
    pub fn hits(&self, at: Point, reach: f32) -> bool {
        geometry::hits(self, at, reach)
    }

    #[must_use]
    pub fn moved(&self, by: Point) -> Form {
        self.mapped(Affine::translation(f64::from(by.x), f64::from(by.y)), false)
    }

    #[must_use]
    pub fn mapped(&self, map: Affine, strokes: bool) -> Form {
        geometry::mapped(self, map, strokes)
    }

    #[must_use]
    pub fn is_worth_keeping(&self) -> bool {
        geometry::is_worth_keeping(self)
    }
}
