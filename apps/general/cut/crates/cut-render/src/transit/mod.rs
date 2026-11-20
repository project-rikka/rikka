mod camera;
mod dissolve;
mod motion;
mod stylize;
mod swatch;
mod wipe;

use cut_project::{
    Direction, Transition, TransitionFamily as Family, TransitionKind as Kind, Tuning,
};
use rikka_image_edit::{Mask, Picture, Place, Recipe, Result, Step};

use crate::plan::IDENTITY;

pub use swatch::Swatch;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mix {
    pub(crate) kind: Kind,
    pub(crate) progress: f32,
    pub(crate) before: f32,
    pub(crate) after: f32,
    pub(crate) linear: f32,
    pub(crate) step: i64,
    pub(crate) direction: Direction,
    pub(crate) tuning: Tuning,
    pub(crate) seed: u32,
    pub(crate) factor: f32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) fills: [bool; 2],
    pub(crate) clear: Picture,
    pub(crate) deep: Option<Picture>,
}

pub(crate) struct Backdrops {
    pub(crate) clear: Picture,
    pub(crate) deep: Option<Picture>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Timing {
    pub(crate) step: i64,
    pub(crate) length: i64,
    pub(crate) seed: u32,
}

impl Mix {
    pub(crate) fn new(
        transition: &Transition,
        timing: Timing,
        factor: f32,
        fills: [bool; 2],
        backdrops: Backdrops,
    ) -> Mix {
        let length = timing.length.max(1) as f64;
        let middle = timing.step as f64 + 0.5;
        let linear = |at: f64| (at / length).clamp(0.0, 1.0);
        let eased = |at: f64| transition.easing.at(linear(at)) as f32;
        let (width, height) = backdrops.clear.dimensions();
        Mix {
            kind: transition.kind,
            progress: eased(middle),
            before: eased(middle - 0.5),
            after: eased(middle + 0.5),
            linear: linear(middle) as f32,
            step: timing.step,
            direction: transition.direction,
            tuning: transition.tuning,
            seed: timing.seed,
            factor,
            width,
            height,
            fills,
            clear: backdrops.clear,
            deep: backdrops.deep,
        }
    }

    pub(crate) fn render(&self, out: &Picture, into: &Picture) -> Result<Picture> {
        match self.kind.family() {
            Family::Dissolve => dissolve::render(self, out, into),
            Family::Wipe => wipe::render(self, out, into),
            Family::Motion => motion::render(self, out, into),
            Family::Camera => camera::render(self, out, into),
            Family::Stylize => stylize::render(self, out, into),
        }
    }

    fn centre(&self) -> (f32, f32) {
        (self.width as f32 / 2.0, self.height as f32 / 2.0)
    }

    fn reach(&self) -> f32 {
        self.width.max(self.height) as f32
    }

    fn travel(&self) -> f32 {
        match self.direction {
            Direction::Up | Direction::Down => self.height as f32,
            _ => self.width as f32,
        }
    }

    fn shifted(&self, by: f32) -> [[f32; 3]; 3] {
        let along = self.direction.vector();
        self.about(Change {
            shift: (along.x * by, along.y * by),
            ..Change::NONE
        })
    }

    fn per_frame(&self, shape: impl Fn(f32) -> f32) -> f32 {
        (shape(self.after) - shape(self.before)).abs()
    }

    fn shutter(&self) -> f32 {
        self.tuning.shutter / 100.0
    }

    fn sweep(&self) -> f32 {
        self.travel() * self.per_frame(|progress| progress) * self.shutter()
    }

    fn amount(&self) -> f32 {
        self.tuning.amount
    }

    fn strength(&self) -> f32 {
        self.tuning.amount / 100.0
    }

    fn peak(&self) -> f32 {
        1.0 - (2.0 * self.progress - 1.0).abs()
    }

    fn swell(&self) -> f32 {
        (std::f32::consts::PI * self.linear).sin()
    }

    fn side<'p>(&self, out: &'p Picture, into: &'p Picture) -> (&'p Picture, bool) {
        match self.progress < 0.5 {
            true => (out, self.fills[0]),
            false => (into, self.fills[1]),
        }
    }

    fn mixed(
        &self,
        base: &Picture,
        top: &Picture,
        weight: f32,
        mask: Option<Mask>,
    ) -> Result<Picture> {
        if weight <= 0.0 && mask.is_none() {
            return Ok(base.clone());
        }
        let step = Step::new(Place::new(top.clone(), IDENTITY).replacing()).opacity(weight);
        let step = match mask {
            Some(mask) => step.masked(mask),
            None => step,
        };
        base.develop(&Recipe::new().then(step))
    }

    fn moved(&self, picture: &Picture, matrix: [[f32; 3]; 3], mirrored: bool) -> Result<Picture> {
        let place = Place::new(picture.clone(), matrix);
        let place = match mirrored {
            true => place.mirrored(),
            false => place,
        };
        self.clear.develop(&Recipe::new().then(place))
    }

    fn about(&self, change: Change) -> [[f32; 3]; 3] {
        let (cx, cy) = self.centre();
        let (sin, cos) = change.turn.to_radians().sin_cos();
        let (sx, sy) = change.scale;
        let (dx, dy) = change.shift;
        [
            [cos * sx, -sin * sy, cx + dx - cos * sx * cx + sin * sy * cy],
            [sin * sx, cos * sy, cy + dy - sin * sx * cx - cos * sy * cy],
            [0.0, 0.0, 1.0],
        ]
    }
}

#[derive(Clone, Copy, Debug)]
struct Change {
    scale: (f32, f32),
    turn: f32,
    shift: (f32, f32),
}

impl Change {
    const NONE: Change = Change {
        scale: (1.0, 1.0),
        turn: 0.0,
        shift: (0.0, 0.0),
    };

    fn zoom(scale: f32) -> Change {
        Change {
            scale: (scale, scale),
            ..Change::NONE
        }
    }
}

fn smooth(from: f32, to: f32, at: f32) -> f32 {
    let t = ((at - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn degrees(direction: Direction) -> f32 {
    let vector = direction.vector();
    (-vector.y).atan2(vector.x).to_degrees()
}

fn hashed(seed: u32, salt: u64) -> f32 {
    let mut z = (u64::from(seed) << 32 ^ salt).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}
