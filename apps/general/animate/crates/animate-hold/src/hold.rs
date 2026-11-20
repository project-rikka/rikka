use animate_body::Side;
use rikka_model_core::glam::Vec3;

const EASE: u32 = 6;
pub const MAX_EASE: u32 = 120;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Holder {
    Hand(Side),
    Pin { node: u32, at: Vec3 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct HoldTarget {
    pub figure: Option<String>,
    pub at: Vec3,
    pub reach: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hold {
    pub holder: Holder,
    pub target: HoldTarget,
    pub from: u32,
    pub until: Option<u32>,
    pub ease: u32,
}

impl Hold {
    #[must_use]
    pub fn new(holder: Holder, target: HoldTarget, from: u32) -> Self {
        Self {
            holder,
            target,
            from,
            until: None,
            ease: EASE,
        }
    }

    #[must_use]
    pub fn hand(&self) -> Option<Side> {
        match self.holder {
            Holder::Hand(side) => Some(side),
            Holder::Pin { .. } => None,
        }
    }

    #[must_use]
    pub fn holds_at(&self, frame: u32) -> bool {
        self.from <= frame && self.until.is_none_or(|until| frame < until)
    }

    #[must_use]
    pub fn strength(&self, frame: f32) -> f32 {
        let ease = self.ease as f32;
        let rising = ramp(frame - (self.from as f32 - ease), self.ease);
        let falling = self.until.map_or(1.0, |until| {
            1.0 - ramp(frame - (until as f32 - ease), self.ease)
        });
        rising.min(falling)
    }

    #[must_use]
    pub fn closes_at(&self) -> u32 {
        self.from.saturating_sub(self.ease)
    }

    pub(crate) fn overlaps(&self, other: &Hold) -> bool {
        let ends_after = |hold: &Hold, frame: u32| hold.until.is_none_or(|until| until > frame);
        ends_after(self, other.from) && ends_after(other, self.from)
    }
}

fn ramp(elapsed: f32, ease: u32) -> f32 {
    if elapsed < 0.0 {
        return 0.0;
    }
    if ease == 0 {
        return 1.0;
    }
    let share = (elapsed / ease as f32).min(1.0);
    share * share * (3.0 - 2.0 * share)
}
