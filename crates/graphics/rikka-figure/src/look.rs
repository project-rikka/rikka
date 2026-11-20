use rikka_image_edit::{Cap, FillRule, Join, Path, PathMark, ShapeStyle, StrokeStyle};
use rikka_types::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    pub color: Color,
    pub width: f32,
}

impl Pen {
    #[must_use]
    pub fn new(color: Color, width: f32) -> Pen {
        Pen { color, width }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub fill: Option<Color>,
    pub outline: Option<Pen>,
}

impl Look {
    #[must_use]
    pub fn styled(ink: Color, width: f32, style: ShapeStyle) -> Look {
        let pen = Pen::new(ink, width);
        match style {
            ShapeStyle::Outline => Look {
                fill: None,
                outline: Some(pen),
            },
            ShapeStyle::Fill => Look {
                fill: Some(ink),
                outline: None,
            },
            ShapeStyle::FillAndOutline { fill } => Look {
                fill: Some(fill),
                outline: Some(pen),
            },
        }
    }

    #[must_use]
    pub fn style(&self) -> ShapeStyle {
        match (self.fill, self.outline) {
            (Some(fill), Some(_)) => ShapeStyle::FillAndOutline { fill },
            (Some(_), None) => ShapeStyle::Fill,
            (None, _) => ShapeStyle::Outline,
        }
    }

    #[must_use]
    pub fn ink(&self) -> Color {
        match (self.outline, self.fill) {
            (Some(pen), _) => pen.color,
            (None, Some(fill)) => fill,
            (None, None) => Color::TRANSPARENT,
        }
    }

    #[must_use]
    pub fn mark(&self, path: Path) -> PathMark {
        let mut figure = PathMark::new(path);
        if let Some(fill) = self.fill {
            figure = figure.filled(fill, FillRule::NonZero);
        }
        if let Some(pen) = self.outline {
            figure = figure.stroked(
                pen.color,
                StrokeStyle::new(pen.width)
                    .join(Join::Round)
                    .cap(Cap::Round),
            );
        }
        figure
    }

    #[must_use]
    pub fn reach(&self) -> f32 {
        self.outline.map_or(0.0, |pen| pen.width / 2.0)
    }
}
