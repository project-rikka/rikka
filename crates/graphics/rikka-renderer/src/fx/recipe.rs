use std::hash::{Hash, Hasher};

use rikka_types::{Lens, Rect, RefractionExtent, RefractionProfile, RoundedRect};

use crate::fx::effect::{
    BackdropPass, PassList, PostEffect, accumulate_reach, core_reach, core_spread_of,
    first_write_rounding, spread_of,
};
use crate::fx::field::{Insets, RefractionField};
use crate::fx::joined::Joined;
use crate::fx::refract;

#[derive(Clone, Copy)]
pub(crate) struct Recipe<'a> {
    pub effect: &'a dyn PostEffect,
    pub shape: RoundedRect,
    pub lens: Option<Lens>,
    pub joined: Option<Joined>,
}

impl<'a> Recipe<'a> {
    pub(crate) fn whole(effect: &'a dyn PostEffect, region: Rect) -> Recipe<'a> {
        Recipe {
            effect,
            shape: RoundedRect::rect(region),
            lens: None,
            joined: None,
        }
    }

    pub(crate) fn plan(&self, viewport: Rect) -> Plan {
        self.optics().plan(viewport)
    }

    pub(crate) fn optics(&self) -> Optics {
        let mut list = PassList::default();
        let size = self.shape.rect.size;
        self.effect.passes(size, &mut list);
        let softened = spread_of(&list.passes);
        let lens = self.lens.filter(|lens| lens.refracts());
        let joined = self.joined.filter(|_| lens.is_some());
        let (extent, profile) = match (lens, joined) {
            (Some(lens), Some(joined)) => {
                let (glass, pane) = (joined.host().rect, self.shape.rect);
                (
                    lens.extent_within(glass, pane),
                    lens.profile_within(glass, pane),
                )
            }
            (Some(lens), None) => (lens.extent_for_size(size), lens.profile_for_size(size)),
            (None, _) => (RefractionExtent::NONE, RefractionProfile::NONE),
        };
        let mut passes = std::mem::take(&mut list.passes);
        if let Some(lens) = lens {
            refract::bend(lens, self.shape, joined, profile, &mut passes);
        }
        accumulate_reach(&mut passes);
        let rounding = lens.filter(|_| joined.is_none()).map(|_| Rounding {
            shape: self.shape,
            reach: core_reach(&passes),
        });
        let refraction = lens.and_then(|lens| {
            let field = match joined {
                Some(_) => RefractionField::isotropic(self.shape, lens, profile),
                None => RefractionField::directional(self.shape, lens, profile),
            };
            Some(Refraction {
                field: field?,
                after: passes[0].stale_reach,
            })
        });
        let reads = match joined {
            Some(_) => softened + extent.sample_margin,
            None => softened.max(extent.sample_margin),
        };
        Optics {
            pane: self.shape.rect,
            rounding,
            reads,
            near: softened + profile.bulge_displacement,
            refraction,
            passes,
        }
    }
}

pub(crate) struct Optics {
    pub(crate) pane: Rect,
    pub(crate) rounding: Option<Rounding>,
    pub(crate) passes: Vec<BackdropPass>,
    pub(crate) reads: f32,
    pub(crate) near: f32,
    pub(crate) refraction: Option<Refraction>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Refraction {
    pub(crate) after: f32,
    pub(crate) field: RefractionField,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Rounding {
    pub(crate) shape: RoundedRect,
    reach: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StaleCorners([Option<Rect>; 4]);

impl StaleCorners {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Rect> {
        self.0.iter().flatten()
    }
}

impl Optics {
    pub(crate) fn plan(self, viewport: Rect) -> Plan {
        let source_reach = core_spread_of(&self.passes) + first_write_rounding(&self.passes);
        Plan {
            region: self
                .pane
                .inflate(self.reads)
                .round_out()
                .intersection(&viewport)
                .unwrap_or(self.pane),
            signature: signature_of(&self.passes),
            rounding: self.rounding,
            source_reach,
            refraction: self.refraction,
            passes: self.passes,
        }
    }
}

pub(crate) struct Plan {
    pub(crate) passes: Vec<BackdropPass>,
    pub(crate) region: Rect,
    pub(crate) signature: u64,
    pub(crate) rounding: Option<Rounding>,
    source_reach: f32,
    refraction: Option<Refraction>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StaleRegion {
    pub(crate) rect: Rect,
    displacement: Insets,
}

impl StaleRegion {
    pub(crate) fn whole(rect: Rect) -> StaleRegion {
        StaleRegion {
            rect,
            displacement: Insets::default(),
        }
    }
}

impl Plan {
    pub(crate) fn reshaped(&self, cached_shape: Option<RoundedRect>) -> StaleCorners {
        let mut reshaped = StaleCorners::default();
        let Some((rounding, was)) = self.rounding.zip(cached_shape) else {
            return reshaped;
        };
        let corners = rounding.shape.changed_corners(&was).into_iter().flatten();
        let stale = corners.filter_map(|corner| {
            corner
                .inflate(rounding.reach)
                .round_out()
                .intersection(&self.region)
        });
        for (slot, rect) in reshaped.0.iter_mut().zip(stale) {
            *slot = Some(rect);
        }
        reshaped
    }

    pub(crate) fn scale(&self) -> Option<f32> {
        Some(self.passes.last()?.scale)
    }

    pub(crate) fn stale_region(&self, rect: Rect) -> StaleRegion {
        StaleRegion {
            rect,
            displacement: self.refraction.map_or_else(Insets::default, |refraction| {
                refraction
                    .field
                    .outward_reach(rect.inflate(refraction.after))
            }),
        }
    }

    pub(crate) fn lift_of(&self, stale: &StaleRegion) -> Rect {
        stale
            .displacement
            .expand(stale.rect.inflate(self.source_reach))
    }

    pub(crate) fn stale_zone(&self, rect: Rect) -> Option<Rect> {
        self.lift_of(&self.stale_region(rect))
            .round_out()
            .intersection(&self.region)
    }

    pub(crate) fn writes(&self, stale: &StaleRegion, index: usize) -> Rect {
        stale.rect.inflate(self.passes[index].core_stale_reach)
    }
}

fn signature_of(passes: &[BackdropPass]) -> u64 {
    let mut hasher = rustc_hash::FxHasher::default();
    for pass in passes {
        pass.shader.hash().hash(&mut hasher);
        pass.scale.to_bits().hash(&mut hasher);
        pass.extent.hash(&mut hasher);
        for value in pass.signed_params() {
            value.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}
