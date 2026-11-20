use rikka_types::Point;

use crate::placement::{Placement, PlacementField};

const QUARTER: f32 = 90.0;

impl Placement {
    #[must_use]
    pub fn slid(self, by: Point) -> Placement {
        if !self.three_d {
            return self.moved(by);
        }
        let (sin_x, cos_x) = self.tilt_x.to_radians().sin_cos();
        let (sin_y, cos_y) = self.tilt_y.to_radians().sin_cos();
        let (across, down, away) = (by.x * cos_y, by.y, -by.x * sin_y);
        Placement {
            x: self.x + across,
            y: self.y + cos_x * down - sin_x * away,
            depth: self.depth + sin_x * down + cos_x * away,
            ..self
        }
    }

    #[must_use]
    pub fn ring(self, field: PlacementField) -> Option<Placement> {
        let (tilt_x, tilt_y) = match field {
            PlacementField::TiltX => (0.0, QUARTER),
            PlacementField::TiltY => (self.tilt_x - QUARTER, 0.0),
            PlacementField::Rotation => (self.tilt_x, self.tilt_y),
            _ => return None,
        };
        self.three_d.then(|| {
            Placement {
                anchor: Point::ZERO,
                zoom: 1.0,
                aspect: 0.0,
                rotation: 0.0,
                tilt_x,
                tilt_y,
                ..self
            }
            .checked()
        })
    }
}
