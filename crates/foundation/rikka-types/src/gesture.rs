use crate::point::Point;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    Swipe {
        phase: GesturePhase,
        fingers: u32,
        delta: Point,
    },
    Pinch {
        phase: GesturePhase,
        fingers: u32,
        delta: Point,
        scale: f32,
        rotation: f32,
    },
    Hold {
        phase: GesturePhase,
        fingers: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    Began,
    Moved,
    Ended,
    Cancelled,
}

impl Gesture {
    #[must_use]
    pub fn phase(self) -> GesturePhase {
        match self {
            Gesture::Swipe { phase, .. }
            | Gesture::Pinch { phase, .. }
            | Gesture::Hold { phase, .. } => phase,
        }
    }

    #[must_use]
    pub fn fingers(self) -> u32 {
        match self {
            Gesture::Swipe { fingers, .. }
            | Gesture::Pinch { fingers, .. }
            | Gesture::Hold { fingers, .. } => fingers,
        }
    }
}
