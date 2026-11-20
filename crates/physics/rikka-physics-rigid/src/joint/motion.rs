#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    LinearX,
    LinearY,
    LinearZ,
    SwingX,
    SwingY,
    Twist,
}

impl Axis {
    pub const ALL: [Axis; 6] = [
        Axis::LinearX,
        Axis::LinearY,
        Axis::LinearZ,
        Axis::SwingX,
        Axis::SwingY,
        Axis::Twist,
    ];

    pub const LINEAR: [Axis; 3] = [Axis::LinearX, Axis::LinearY, Axis::LinearZ];

    pub const ANGULAR: [Axis; 3] = [Axis::SwingX, Axis::SwingY, Axis::Twist];

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub fn is_linear(self) -> bool {
        self.index() < 3
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    Free,
    Locked,
    Limited { lower: f32, upper: f32 },
}

impl Motion {
    pub fn from_bounds(lower: f32, upper: f32) -> Self {
        if lower > upper {
            Motion::Free
        } else {
            Motion::Limited { lower, upper }
        }
    }

    pub(crate) fn encode(self) -> (f32, f32) {
        match self {
            Motion::Free => (f32::INFINITY, f32::NEG_INFINITY),
            Motion::Locked => (0.0, 0.0),
            Motion::Limited { lower, upper } if lower <= upper => (lower, upper),
            Motion::Limited { .. } => (f32::NAN, f32::NAN),
        }
    }

    pub(crate) fn decode(lower: f32, upper: f32) -> Self {
        if lower == f32::INFINITY && upper == f32::NEG_INFINITY {
            Motion::Free
        } else if lower == 0.0 && upper == 0.0 {
            Motion::Locked
        } else {
            Motion::Limited { lower, upper }
        }
    }

    pub(crate) fn bounds(self) -> Option<(f32, f32)> {
        match self {
            Motion::Free => None,
            Motion::Locked => Some((0.0, 0.0)),
            Motion::Limited { lower, upper } => Some((lower, upper)),
        }
    }

    pub(crate) fn is_locked(self) -> bool {
        self.bounds().is_some_and(|(lower, upper)| lower == upper)
    }

    pub(crate) fn is_free(self) -> bool {
        self == Motion::Free
    }
}
