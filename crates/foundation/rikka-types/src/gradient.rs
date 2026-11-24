use std::fmt;

use crate::{Color, Point, Rect};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct Gradient {
    angle: f32,
    stops: [GradientStop; Gradient::MAX_STOPS],
    count: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooManyStops;

impl fmt::Display for TooManyStops {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a gradient holds at most {} stops", Gradient::MAX_STOPS)
    }
}

impl std::error::Error for TooManyStops {}

impl Gradient {
    pub const MAX_STOPS: usize = 8;

    pub fn linear(angle: f32, from: Color, to: Color) -> Self {
        let mut stops = [GradientStop {
            offset: 1.0,
            color: to,
        }; Gradient::MAX_STOPS];
        stops[0] = GradientStop {
            offset: 0.0,
            color: from,
        };
        Self {
            angle,
            stops,
            count: 2,
        }
    }

    pub fn with_stop(mut self, offset: f32, color: Color) -> Result<Self, TooManyStops> {
        let count = usize::from(self.count);
        if count == Gradient::MAX_STOPS {
            return Err(TooManyStops);
        }
        let offset = offset.clamp(0.0, 1.0);
        let at = self.stops[..count].partition_point(|stop| stop.offset <= offset);
        self.stops.copy_within(at..count, at + 1);
        self.stops[at] = GradientStop { offset, color };
        self.count += 1;
        Ok(self)
    }

    #[must_use]
    pub fn stops(&self) -> &[GradientStop] {
        &self.stops[..usize::from(self.count)]
    }

    #[must_use]
    pub fn line(&self, rect: Rect) -> (Point, Point) {
        let (sin, cos) = self.angle.sin_cos();
        let reach = (rect.width() * sin.abs() + rect.height() * cos.abs()) * 0.5;
        let center = rect.center();
        let along = Point::new(sin * reach, -cos * reach);
        (
            Point::new(center.x - along.x, center.y - along.y),
            Point::new(center.x + along.x, center.y + along.y),
        )
    }

    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.stops().iter().all(|stop| stop.color.a >= 1.0)
    }

    pub fn scale_alpha(mut self, factor: f32) -> Self {
        for stop in &mut self.stops {
            stop.color = stop.color.scale_alpha(factor);
        }
        self
    }
}
