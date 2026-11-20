use rikka_types::Color;

use crate::marks::path::Path;

const MITER_LIMIT: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub join: Join,
    pub cap: Cap,
    pub miter_limit: f32,
}

impl StrokeStyle {
    #[must_use]
    pub fn new(width: f32) -> StrokeStyle {
        StrokeStyle {
            width,
            join: Join::default(),
            cap: Cap::default(),
            miter_limit: MITER_LIMIT,
        }
    }

    #[must_use]
    pub fn join(mut self, join: Join) -> StrokeStyle {
        self.join = join;
        self
    }

    #[must_use]
    pub fn cap(mut self, cap: Cap) -> StrokeStyle {
        self.cap = cap;
        self
    }

    #[must_use]
    pub fn miter_limit(mut self, limit: f32) -> StrokeStyle {
        self.miter_limit = limit;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathMark {
    path: Path,
    fill: Option<(Color, FillRule)>,
    stroke: Option<(Color, StrokeStyle)>,
    antialiased: bool,
}

impl PathMark {
    #[must_use]
    pub fn new(path: Path) -> PathMark {
        PathMark {
            path,
            fill: None,
            stroke: None,
            antialiased: true,
        }
    }

    #[must_use]
    pub fn filled(mut self, color: Color, rule: FillRule) -> PathMark {
        self.fill = Some((color, rule));
        self
    }

    #[must_use]
    pub fn stroked(mut self, color: Color, style: StrokeStyle) -> PathMark {
        self.stroke = Some((color, style));
        self
    }

    #[must_use]
    pub fn antialiased(mut self, antialiased: bool) -> PathMark {
        self.antialiased = antialiased;
        self
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn fill(&self) -> Option<(Color, FillRule)> {
        self.fill
    }

    #[must_use]
    pub fn stroke(&self) -> Option<(Color, StrokeStyle)> {
        self.stroke
    }

    #[must_use]
    pub fn is_antialiased(&self) -> bool {
        self.antialiased
    }

    #[must_use]
    pub(crate) fn scaled(&self, factor: f32) -> PathMark {
        PathMark {
            path: self.path.scaled(factor),
            fill: self.fill,
            stroke: self.stroke.map(|(color, style)| {
                (
                    color,
                    StrokeStyle {
                        width: style.width * factor,
                        ..style
                    },
                )
            }),
            antialiased: self.antialiased,
        }
    }
}
