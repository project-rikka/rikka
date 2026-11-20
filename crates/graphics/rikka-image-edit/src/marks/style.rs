use rikka_types::Color;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ShapeStyle {
    #[default]
    Outline,
    Fill,
    FillAndOutline {
        fill: Color,
    },
}

impl ShapeStyle {
    #[must_use]
    pub(crate) fn fill(self, ink: Color) -> Option<Color> {
        match self {
            ShapeStyle::Outline => None,
            ShapeStyle::Fill => Some(ink),
            ShapeStyle::FillAndOutline { fill } => Some(fill),
        }
    }

    #[must_use]
    pub(crate) fn outlines(self) -> bool {
        !matches!(self, ShapeStyle::Fill)
    }
}
