use rikka_types::Point;

use super::direction::Direction;
use crate::placement::Placement;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wipe {
    pub direction: Direction,
    pub progress: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresetTransform {
    pub offset: Point,
    pub zoom: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub blur: f32,
    pub wipe: Option<Wipe>,
    pub reveal: f32,
}

impl Default for PresetTransform {
    fn default() -> PresetTransform {
        PresetTransform::REST
    }
}

impl PresetTransform {
    pub const REST: PresetTransform = PresetTransform {
        offset: Point::ZERO,
        zoom: 1.0,
        rotation: 0.0,
        opacity: 1.0,
        blur: 0.0,
        wipe: None,
        reveal: 1.0,
    };

    #[must_use]
    pub fn is_rest(&self) -> bool {
        *self == PresetTransform::REST
    }

    #[must_use]
    pub fn placed(&self, placement: Placement) -> Placement {
        Placement {
            x: placement.x + self.offset.x,
            y: placement.y + self.offset.y,
            zoom: placement.zoom * self.zoom,
            rotation: placement.rotation + self.rotation,
            opacity: placement.opacity * self.opacity,
            ..placement
        }
    }

    pub(crate) fn then(self, other: PresetTransform) -> PresetTransform {
        PresetTransform {
            offset: self.offset + other.offset,
            zoom: self.zoom * other.zoom,
            rotation: self.rotation + other.rotation,
            opacity: self.opacity * other.opacity,
            blur: self.blur + other.blur,
            wipe: match (self.wipe, other.wipe) {
                (Some(left), Some(right)) if right.progress < left.progress => Some(right),
                (Some(left), _) => Some(left),
                (None, right) => right,
            },
            reveal: self.reveal * other.reveal,
        }
    }
}
