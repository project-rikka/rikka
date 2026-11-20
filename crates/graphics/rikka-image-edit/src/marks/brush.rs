use rikka_types::{Color, Point};

const MIN_STEP: f32 = 1.0;

const MIN_SIZE: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    pub size: f32,
    pub hardness: f32,
    pub opacity: f32,
    pub flow: f32,
    pub antialiased: bool,
}

impl Brush {
    #[must_use]
    pub fn new(size: f32) -> Brush {
        Brush {
            size,
            hardness: 1.0,
            opacity: 1.0,
            flow: 1.0,
            antialiased: true,
        }
    }

    #[must_use]
    pub(crate) fn clamped(&self) -> Brush {
        Brush {
            size: self.size.max(MIN_SIZE),
            hardness: self.hardness.clamp(0.0, 1.0),
            opacity: self.opacity.clamp(0.0, 1.0),
            flow: self.flow.clamp(0.0, 1.0),
            antialiased: self.antialiased,
        }
    }

    #[must_use]
    pub(crate) fn scaled(&self, factor: f32) -> Brush {
        Brush {
            size: self.size * factor,
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Paint(Color),
    Erase { background: Color },
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushStroke {
    brush: Brush,
    ink: Ink,
    points: Vec<Point>,
}

impl BrushStroke {
    #[must_use]
    pub fn new(brush: Brush, ink: Ink, from: Point) -> BrushStroke {
        BrushStroke {
            brush,
            ink,
            points: vec![from],
        }
    }

    #[must_use]
    pub fn from_points(
        brush: Brush,
        ink: Ink,
        points: impl IntoIterator<Item = Point>,
    ) -> Option<BrushStroke> {
        let mut points = points.into_iter();
        let mut stroke = BrushStroke::new(brush, ink, points.next()?);
        for point in points {
            stroke.extend_to(point);
        }
        Some(stroke)
    }

    pub fn extend_to(&mut self, at: Point) -> bool {
        if self
            .points
            .last()
            .is_some_and(|last| last.distance(at) < MIN_STEP)
        {
            return false;
        }
        self.points.push(at);
        true
    }

    #[must_use]
    pub fn brush(&self) -> Brush {
        self.brush
    }

    #[must_use]
    pub fn ink(&self) -> Ink {
        self.ink
    }

    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    #[must_use]
    pub fn erases(&self) -> bool {
        matches!(self.ink, Ink::Erase { .. })
    }

    #[must_use]
    pub(crate) fn scaled(&self, factor: f32) -> BrushStroke {
        BrushStroke {
            brush: self.brush.scaled(factor),
            ink: self.ink,
            points: self.points.iter().map(|point| *point * factor).collect(),
        }
    }
}
