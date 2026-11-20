use rikka_image_edit::Tool;
use rikka_types::Color;
use rikka_ui::{Label, MaterialSymbol, tr};

pub(super) const INKS: [Color; 6] = [
    Color::rgb(0.90, 0.15, 0.15),
    Color::rgb(1.00, 0.76, 0.06),
    Color::rgb(0.18, 0.74, 0.35),
    Color::rgb(0.16, 0.47, 0.96),
    Color::rgb(0.05, 0.05, 0.07),
    Color::WHITE,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    Draw(Tool),
    Erase,
}

impl Default for Pick {
    fn default() -> Pick {
        Pick::Draw(Tool::Pen)
    }
}

impl Pick {
    pub(crate) const ALL: [Pick; 7] = [
        Pick::Draw(Tool::Pen),
        Pick::Draw(Tool::Highlighter),
        Pick::Draw(Tool::Line),
        Pick::Draw(Tool::Arrow),
        Pick::Draw(Tool::Rectangle),
        Pick::Draw(Tool::Ellipse),
        Pick::Erase,
    ];

    #[must_use]
    pub(super) fn from_index(index: usize) -> Option<Pick> {
        Pick::ALL.get(index).copied()
    }

    #[must_use]
    pub(crate) fn index(self) -> usize {
        Pick::ALL
            .iter()
            .position(|known| *known == self)
            .unwrap_or_default()
    }

    #[must_use]
    pub(crate) fn symbol(self) -> MaterialSymbol {
        match self {
            Pick::Draw(Tool::Pen) => MaterialSymbol::DRAW,
            Pick::Draw(Tool::Highlighter) => MaterialSymbol::INK_HIGHLIGHTER,
            Pick::Draw(Tool::Line) => MaterialSymbol::HORIZONTAL_RULE,
            Pick::Draw(Tool::Arrow) => MaterialSymbol::ARROW_OUTWARD,
            Pick::Draw(Tool::Rectangle) => MaterialSymbol::RECTANGLE,
            Pick::Draw(Tool::Ellipse) => MaterialSymbol::PANORAMA_FISH_EYE,
            Pick::Erase => MaterialSymbol::INK_ERASER,
        }
    }

    #[must_use]
    pub(crate) fn label(self) -> Label {
        match self {
            Pick::Draw(Tool::Pen) => tr!("shell.markup.tool.pen"),
            Pick::Draw(Tool::Highlighter) => tr!("shell.markup.tool.highlighter"),
            Pick::Draw(Tool::Line) => tr!("shell.markup.tool.line"),
            Pick::Draw(Tool::Arrow) => tr!("shell.markup.tool.arrow"),
            Pick::Draw(Tool::Rectangle) => tr!("shell.markup.tool.rectangle"),
            Pick::Draw(Tool::Ellipse) => tr!("shell.markup.tool.ellipse"),
            Pick::Erase => tr!("shell.markup.tool.eraser"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Width {
    Thin,
    #[default]
    Medium,
    Thick,
}

impl Width {
    pub(crate) const ALL: [Width; 3] = [Width::Thin, Width::Medium, Width::Thick];

    #[must_use]
    pub(super) fn from_index(index: usize) -> Option<Width> {
        Width::ALL.get(index).copied()
    }

    #[must_use]
    pub(crate) fn index(self) -> usize {
        Width::ALL
            .iter()
            .position(|known| *known == self)
            .unwrap_or_default()
    }

    #[must_use]
    pub(crate) fn pixels(self) -> f32 {
        match self {
            Width::Thin => 3.0,
            Width::Medium => 6.0,
            Width::Thick => 12.0,
        }
    }

    #[must_use]
    pub(crate) fn label(self) -> Label {
        match self {
            Width::Thin => tr!("shell.markup.width.thin"),
            Width::Medium => tr!("shell.markup.width.medium"),
            Width::Thick => tr!("shell.markup.width.thick"),
        }
    }
}
