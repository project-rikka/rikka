use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_figure::{CanvasTransform, Form, FormKind, Look, Pen, ShapeKind};
use rikka_image_edit::{Recipe, ShapeStyle};
use rikka_types::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FigureId(pub u64);

#[derive(Clone, Debug)]
pub struct FigureIds(Arc<AtomicU64>);

impl Default for FigureIds {
    fn default() -> FigureIds {
        FigureIds(Arc::new(AtomicU64::new(1)))
    }
}

impl FigureIds {
    #[must_use]
    pub fn next(&self) -> FigureId {
        FigureId(self.0.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn claim(&self, id: FigureId) {
        self.0.fetch_max(id.0.saturating_add(1), Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FigureKind {
    Line,
    Arrow,
    Rectangle,
    Ellipse,
    Path,
    Text,
}

impl FigureKind {
    #[must_use]
    pub fn from_form_kind(kind: FormKind) -> FigureKind {
        match kind {
            FormKind::Line => FigureKind::Line,
            FormKind::Arrow => FigureKind::Arrow,
            FormKind::Shape(ShapeKind::Ellipse) => FigureKind::Ellipse,
            FormKind::Shape(_) => FigureKind::Rectangle,
            FormKind::Path => FigureKind::Path,
            FormKind::Text => FigureKind::Text,
        }
    }

    #[must_use]
    pub fn form_kind(self) -> FormKind {
        match self {
            FigureKind::Line => FormKind::Line,
            FigureKind::Arrow => FormKind::Arrow,
            FigureKind::Rectangle => FormKind::Shape(ShapeKind::Rectangle),
            FigureKind::Ellipse => FormKind::Shape(ShapeKind::Ellipse),
            FigureKind::Path => FormKind::Path,
            FigureKind::Text => FormKind::Text,
        }
    }

    #[must_use]
    pub fn spanned(self, from: Point, to: Point, pen: Pen, style: ShapeStyle) -> Option<Form> {
        let look = match self {
            FigureKind::Line | FigureKind::Arrow => Look {
                fill: None,
                outline: Some(pen),
            },
            _ => Look::styled(pen.color, pen.width, style),
        };
        Form::spanned(self.form_kind(), from, to, look)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub id: FigureId,
    pub form: Form,
}

impl Figure {
    #[must_use]
    pub fn new(id: FigureId, form: Form) -> Figure {
        Figure { id, form }
    }

    #[must_use]
    pub fn kind(&self) -> FigureKind {
        FigureKind::from_form_kind(self.form.kind())
    }

    #[must_use]
    pub fn with(&self, form: Form) -> Figure {
        Figure { id: self.id, form }
    }
}

pub(crate) fn recipe(figures: &[Figure]) -> Recipe {
    rikka_figure::recipe(
        figures.iter().map(|figure| &figure.form),
        CanvasTransform::IDENTITY,
    )
}
