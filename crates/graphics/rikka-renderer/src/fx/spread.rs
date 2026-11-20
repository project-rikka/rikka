use rikka_types::{Lens, Rect, RoundedRect};

use crate::fx::effect::PostEffect;
use crate::fx::joined::Joined;
use crate::fx::recipe::{Optics, Recipe, Refraction};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spread {
    pane: Rect,
    reads: f32,
    near: f32,
    edge: f32,
    refraction: Option<Refraction>,
}

impl Spread {
    pub fn new(
        shape: RoundedRect,
        effect: &dyn PostEffect,
        lens: Option<Lens>,
        joined: Option<Joined>,
    ) -> Spread {
        Spread::from(
            &Recipe {
                effect,
                shape,
                lens,
                joined,
            }
            .optics(),
        )
    }

    pub fn reads(&self) -> f32 {
        self.reads
    }

    pub fn sensitive_region(&self) -> Rect {
        self.pane.inflate(self.reads)
    }

    pub fn stale_regions(&self, changed: Rect, stale: &mut Vec<Rect>) {
        let Some(seen) = changed.intersection(&self.sensitive_region()) else {
            return;
        };
        let shown = self.pane.inflate(self.edge);
        stale.extend(seen.inflate(self.near).intersection(&shown));
        let Some(refraction) = self.refraction else {
            return;
        };
        let start = stale.len();
        refraction.field.displaced_sources(seen, stale);
        let mut count = start;
        for index in start..stale.len() {
            if let Some(reached) = stale[index].inflate(refraction.after).intersection(&shown) {
                stale[count] = reached;
                count += 1;
            }
        }
        stale.truncate(count);
    }
}

impl From<&Optics> for Spread {
    fn from(optics: &Optics) -> Spread {
        Spread {
            pane: optics.pane,
            reads: optics.reads,
            near: optics.near,
            edge: optics.passes.last().map_or(0.0, |pass| 2.0 / pass.scale) + 2.0,
            refraction: optics.refraction,
        }
    }
}
