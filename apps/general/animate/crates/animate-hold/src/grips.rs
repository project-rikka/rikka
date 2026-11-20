use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rikka_mmd::FRAMES_PER_SECOND;
use rikka_model_core::Instance;
use rikka_model_core::glam::Vec3;
use rikka_physics_tailor::ClothAsset;
use rikka_physics_tailor::{Grab, MAX_GRABS};
use rustc_hash::FxHasher;

use crate::hold::{Hold, Holder};
use crate::holds::{HoldId, Holds};
use crate::pinches::Pinches;

#[derive(Clone, Copy, Debug)]
pub struct Performer<'a> {
    pub key: u64,
    pub name: &'a str,
    pub instance: &'a Instance,
    pub pinches: Pinches,
    pub holds: Option<&'a Holds>,
    pub cloth_asset: Option<&'a Arc<ClothAsset>>,
}

#[derive(Clone, Debug)]
pub struct ActiveGrab {
    pub owner: usize,
    pub grab: Grab,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GripPoint {
    pub id: HoldId,
    pub holder: Holder,
    pub point: Vec3,
}

#[must_use]
pub fn active_grabs(time: f32, performers: &[Performer<'_>]) -> Vec<ActiveGrab> {
    let frame = time * FRAMES_PER_SECOND;
    let mut active_grabs: Vec<ActiveGrab> = Vec::new();
    for (holder, performer) in performers.iter().enumerate() {
        let Some(holds) = performer.holds else {
            continue;
        };
        active_grabs.extend(
            holds
                .active(frame)
                .filter_map(|(_, hold, strength)| grip(performers, holder, hold, strength)),
        );
    }
    active_grabs.sort_by_key(|target| target.owner);
    let mut counted: Option<(usize, usize)> = None;
    active_grabs.retain(|target| {
        let count = match counted {
            Some((owner, count)) if owner == target.owner => count + 1,
            _ => 1,
        };
        counted = Some((target.owner, count));
        count <= MAX_GRABS
    });
    active_grabs
}

#[must_use]
pub fn grip_points(time: f32, performers: &[Performer<'_>], holder: usize) -> Vec<GripPoint> {
    let frame = (time * FRAMES_PER_SECOND).round().max(0.0) as u32;
    let Some(holds) = performers.get(holder).and_then(|performer| performer.holds) else {
        return Vec::new();
    };
    holds
        .holding(frame)
        .filter_map(|(id, hold)| {
            performers[owner_of(performers, holder, hold)?]
                .cloth_asset?
                .pick(hold.target.at, hold.target.reach)?;
            Some(GripPoint {
                id,
                holder: hold.holder,
                point: point_of(&performers[holder], hold.holder)?,
            })
        })
        .collect()
}

fn owner_of(performers: &[Performer<'_>], holder: usize, hold: &Hold) -> Option<usize> {
    match &hold.target.figure {
        None => Some(holder),
        Some(name) => performers
            .iter()
            .enumerate()
            .find(|(at, performer)| *at != holder && performer.name == name)
            .map(|(at, _)| at),
    }
}

fn point_of(performer: &Performer<'_>, holder: Holder) -> Option<Vec3> {
    let instance = performer.instance;
    match holder {
        Holder::Hand(side) => performer.pinches.point(side, instance),
        Holder::Pin { node, at } => {
            let world = instance.worlds().get(node as usize)?;
            Some((instance.transform() * *world).transform_point3(at))
        }
    }
}

fn grip(
    performers: &[Performer<'_>],
    holder: usize,
    hold: &Hold,
    strength: f32,
) -> Option<ActiveGrab> {
    let owner = owner_of(performers, holder, hold)?;
    let grasp = performers[owner]
        .cloth_asset?
        .pick(hold.target.at, hold.target.reach)?;
    Some(ActiveGrab {
        owner,
        grab: Grab {
            id: identity(performers[holder].key, hold),
            pick: Arc::new(grasp),
            point: point_of(&performers[holder], hold.holder)?,
            strength,
        },
    })
}

fn identity(holder: u64, hold: &Hold) -> u64 {
    let mut hasher = FxHasher::default();
    holder.hash(&mut hasher);
    hold.from.hash(&mut hasher);
    match hold.holder {
        Holder::Hand(side) => side.hash(&mut hasher),
        Holder::Pin { node, at } => {
            node.hash(&mut hasher);
            at.to_array().map(f32::to_bits).hash(&mut hasher);
        }
    }
    hold.target
        .at
        .to_array()
        .map(f32::to_bits)
        .hash(&mut hasher);
    hasher.finish()
}
