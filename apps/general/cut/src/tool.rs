use rikka_app::prelude::*;
use rikka_figure::{FormKind, ShapeKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Tool {
    #[default]
    Select,
    Anchor,
    Text,
    Shape(ShapeKind),
    Line,
    Arrow,
    Range(ShapeKind),
}

impl Tool {
    pub(crate) const ALL: [Tool; 11] = [
        Tool::Select,
        Tool::Anchor,
        Tool::Text,
        Tool::Shape(ShapeKind::Rectangle),
        Tool::Shape(ShapeKind::RoundedRectangle),
        Tool::Shape(ShapeKind::Ellipse),
        Tool::Shape(ShapeKind::Triangle),
        Tool::Shape(ShapeKind::Polygon),
        Tool::Shape(ShapeKind::Star),
        Tool::Line,
        Tool::Arrow,
    ];

    pub(crate) fn made() -> impl Iterator<Item = Tool> {
        Tool::ALL
            .into_iter()
            .filter(|tool| !matches!(tool, Tool::Select | Tool::Anchor))
    }

    pub(crate) fn form_kind(self) -> Option<FormKind> {
        match self {
            Tool::Shape(kind) | Tool::Range(kind) => Some(FormKind::Shape(kind)),
            Tool::Line => Some(FormKind::Line),
            Tool::Arrow => Some(FormKind::Arrow),
            Tool::Select | Tool::Anchor | Tool::Text => None,
        }
    }

    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Tool::Select => MaterialSymbol::ARROW_SELECTOR_TOOL,
            Tool::Anchor => MaterialSymbol::ANCHOR,
            Tool::Text => MaterialSymbol::TITLE,
            Tool::Shape(ShapeKind::Rectangle) => MaterialSymbol::RECTANGLE,
            Tool::Shape(ShapeKind::RoundedRectangle) => MaterialSymbol::CHECK_BOX_OUTLINE_BLANK,
            Tool::Shape(ShapeKind::Ellipse) => MaterialSymbol::CIRCLE,
            Tool::Shape(ShapeKind::Triangle) => MaterialSymbol::CHANGE_HISTORY,
            Tool::Shape(ShapeKind::Polygon) => MaterialSymbol::HEXAGON,
            Tool::Shape(ShapeKind::Star) => MaterialSymbol::STAR,
            Tool::Line => MaterialSymbol::DIAGONAL_LINE,
            Tool::Arrow => MaterialSymbol::ARROW_OUTWARD,
            Tool::Range(_) => MaterialSymbol::HIGHLIGHT_ALT,
        }
    }

    pub(crate) fn draws(self) -> bool {
        matches!(
            self,
            Tool::Shape(_) | Tool::Line | Tool::Arrow | Tool::Range(_)
        )
    }
}
