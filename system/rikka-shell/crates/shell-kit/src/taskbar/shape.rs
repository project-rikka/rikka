use rikka_theme::Corner;

use crate::{ExclusiveZone, PointerMode};

pub const DEPTH: f32 = 40.0;

pub const CORNER: f32 = Corner::Window.radius();

const HANG: f32 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape;

impl Shape {
    #[must_use]
    pub fn corner(self) -> f32 {
        CORNER
    }

    #[must_use]
    pub fn height(self) -> f32 {
        DEPTH + HANG
    }

    #[must_use]
    pub fn reserved(self) -> f32 {
        DEPTH
    }

    #[must_use]
    pub fn exclusive(self) -> ExclusiveZone {
        ExclusiveZone::Fixed(DEPTH)
    }

    #[must_use]
    pub fn pointer(self) -> PointerMode {
        PointerMode::Panes
    }
}
