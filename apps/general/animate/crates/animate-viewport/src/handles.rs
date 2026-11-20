use animate_cast::MmdAxis;
use rikka_app::prelude::{Label, Modifiers, Point, Rect, tr};

const CELL_WIDTH: f32 = 35.0;
const CELL_HEIGHT: f32 = 30.0;
const GAP: f32 = 6.0;
const MARGIN: f32 = 10.0;
const SIGN: f32 = 30.0;
const SPACE_WIDTH: f32 = 64.0;

const BONE_TURN: f32 = 0.005;
const BONE_SLIDE: f32 = 0.05;
const BONE_TURN_FAST: f32 = 5.0;
const CAMERA_TURN: f32 = 0.02;
const CAMERA_SLIDE: f32 = 0.05;
const FAST: f32 = 10.0;
const FINE: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Handle {
    Turn(MmdAxis),
    Slide(MmdAxis),
    Space,
}

impl Handle {
    pub(crate) fn name(self) -> Label {
        match self {
            Handle::Turn(axis) => tr!("animate.handles.turn", axis = axis.letter()),
            Handle::Slide(axis) => tr!("animate.handles.slide", axis = axis.letter()),
            Handle::Space => tr!("animate.handles.space"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Turn,
    Slide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Bone,
    Camera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Offer {
    pub target: Option<Target>,
    pub turns: bool,
    pub slides: bool,
}

impl Offer {
    pub(crate) fn allows(&self, handle: Handle) -> bool {
        match handle {
            Handle::Turn(_) => self.target.is_some() && self.turns,
            Handle::Slide(_) => self.target.is_some() && self.slides,
            Handle::Space => self.target == Some(Target::Bone),
        }
    }
}

pub(crate) struct Layout {
    frame: Rect,
}

impl Layout {
    pub(crate) fn new(frame: Rect) -> Self {
        Self { frame }
    }

    fn column(&self, index: usize) -> f32 {
        self.frame.max_x() - MARGIN - (3 - index) as f32 * (CELL_WIDTH + GAP) + GAP
    }

    fn row_top(&self, from_bottom: usize) -> f32 {
        self.frame.max_y() - MARGIN - (from_bottom + 1) as f32 * (CELL_HEIGHT + GAP) + GAP
    }

    pub(crate) fn rect(&self, handle: Handle) -> Rect {
        let cell = |column: usize, from_bottom: usize| {
            Rect::new(
                self.column(column),
                self.row_top(from_bottom),
                CELL_WIDTH,
                CELL_HEIGHT,
            )
        };
        match handle {
            Handle::Turn(axis) => cell(axis as usize, 1),
            Handle::Slide(axis) => cell(axis as usize, 0),
            Handle::Space => {
                let corner = cell(2, 2);
                Rect::new(
                    corner.max_x() - SPACE_WIDTH,
                    corner.y(),
                    SPACE_WIDTH,
                    CELL_HEIGHT,
                )
            }
        }
    }

    pub(crate) fn sign(&self, row: Row) -> Rect {
        let first = self.rect(match row {
            Row::Turn => Handle::Turn(MmdAxis::X),
            Row::Slide => Handle::Slide(MmdAxis::X),
        });
        Rect::new(first.x() - GAP - SIGN, first.y(), SIGN, first.height())
    }

    pub(crate) fn all() -> [Handle; 7] {
        [
            Handle::Turn(MmdAxis::X),
            Handle::Turn(MmdAxis::Y),
            Handle::Turn(MmdAxis::Z),
            Handle::Slide(MmdAxis::X),
            Handle::Slide(MmdAxis::Y),
            Handle::Slide(MmdAxis::Z),
            Handle::Space,
        ]
    }

    pub(crate) fn hit(&self, at: Point, offer: Offer) -> Option<Handle> {
        Self::all()
            .into_iter()
            .filter(|handle| offer.allows(*handle))
            .find(|handle| self.rect(*handle).contains(at))
    }
}

fn pace(modifiers: Modifiers, fast: f32) -> f32 {
    match (modifiers.shift, modifiers.control) {
        (true, _) => fast,
        (false, true) => FINE,
        (false, false) => 1.0,
    }
}

pub(crate) fn bone_turn(rise: f32, modifiers: Modifiers) -> f32 {
    rise * BONE_TURN * pace(modifiers, BONE_TURN_FAST)
}

pub(crate) fn bone_slide(axis: MmdAxis, rise: f32, modifiers: Modifiers) -> f32 {
    let toward = match axis {
        MmdAxis::X | MmdAxis::Y => rise,
        MmdAxis::Z => -rise,
    };
    toward * BONE_SLIDE * pace(modifiers, FAST)
}

pub(crate) fn camera_turn(rise: f32, modifiers: Modifiers) -> f32 {
    -rise * CAMERA_TURN * pace(modifiers, FAST)
}

pub(crate) fn camera_slide(rise: f32, modifiers: Modifiers) -> f32 {
    -rise * CAMERA_SLIDE * pace(modifiers, FAST)
}
