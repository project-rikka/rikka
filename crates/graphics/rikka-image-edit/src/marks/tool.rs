const HIGHLIGHTER_OPACITY: f32 = 0.4;

const HIGHLIGHTER_THICKNESS: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tool {
    #[default]
    Pen,
    Highlighter,
    Line,
    Arrow,
    Rectangle,
    Ellipse,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::Pen,
        Tool::Highlighter,
        Tool::Line,
        Tool::Arrow,
        Tool::Rectangle,
        Tool::Ellipse,
    ];

    #[must_use]
    pub fn is_freehand(self) -> bool {
        matches!(self, Tool::Pen | Tool::Highlighter)
    }

    #[must_use]
    pub fn fills(self) -> bool {
        matches!(self, Tool::Rectangle | Tool::Ellipse)
    }

    #[must_use]
    pub fn thickness(self, width: f32) -> f32 {
        match self {
            Tool::Highlighter => width * HIGHLIGHTER_THICKNESS,
            _ => width,
        }
    }

    #[must_use]
    pub(crate) fn opacity(self) -> f32 {
        match self {
            Tool::Highlighter => HIGHLIGHTER_OPACITY,
            _ => 1.0,
        }
    }
}
