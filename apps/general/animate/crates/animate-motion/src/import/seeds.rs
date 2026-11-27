use std::collections::{HashMap, HashSet};

use animate_body::{Body, HumanBone, muscles};
use rikka_mmd::Pmx;
use rikka_model_core::{Model, NodeId};

use crate::part::Part;

pub(super) struct Seeds {
    frames: HashMap<Part, Vec<u32>>,
}

impl Seeds {
    pub(super) fn new(model: &Model, body: &Body, keyed: &[(NodeId, u32)]) -> Self {
        let root = moved_by(model, body, &[HumanBone::Hips]);
        let mut moved_parts = vec![0_u16; model.nodes.len()];
        let mut seeded = Vec::new();
        for (bit, part) in Part::ALL.into_iter().enumerate() {
            let bones = bones_of(part);
            if bones.is_empty() {
                continue;
            }
            let mut moving = moved_by(model, body, &bones);
            if reached_through(model, &mut moving) {
                moving.extend(root.iter().copied());
            }
            for node in moving {
                moved_parts[node.index()] |= 1 << bit;
            }
            seeded.push((bit, part));
        }
        let mut keyed_frames: [Vec<u32>; Part::ALL.len()] = Default::default();
        for (node, frame) in keyed {
            let parts = moved_parts.get(node.index()).copied().unwrap_or(0);
            for (bit, frames) in keyed_frames.iter_mut().enumerate() {
                if parts & (1 << bit) != 0 {
                    frames.push(*frame);
                }
            }
        }
        let frames = seeded
            .into_iter()
            .map(|(bit, part)| {
                let mut frames = std::mem::take(&mut keyed_frames[bit]);
                frames.sort_unstable();
                frames.dedup();
                (part, frames)
            })
            .collect();
        Self { frames }
    }

    pub(super) fn keyed_frames(&self, part: Part) -> &[u32] {
        self.frames.get(&part).map_or(&[], Vec::as_slice)
    }
}

fn bones_of(part: Part) -> Vec<HumanBone> {
    if part == Part::Root {
        return vec![HumanBone::Hips];
    }
    let mut bones: Vec<HumanBone> = part
        .muscles()
        .into_iter()
        .flatten()
        .map(|index| muscles()[index].bone)
        .collect();
    bones.dedup();
    bones
}

fn moved_by(model: &Model, body: &Body, bones: &[HumanBone]) -> HashSet<NodeId> {
    let mapping = body.mapping();
    let mut moving = HashSet::new();
    for bone in bones {
        let Some(node) = mapping.node(*bone) else {
            continue;
        };
        let stop = body.parent(*bone).and_then(|parent| mapping.node(parent));
        let mut next = Some(node);
        while let Some(node) = next.filter(|node| Some(*node) != stop && moving.insert(*node)) {
            next = model.node(node).parent;
        }
        moving.extend(mapping.twist(*bone));
    }
    moving
}

fn reached_through(model: &Model, moving: &mut HashSet<NodeId>) -> bool {
    let Some(pmx) = model.extensions.get::<Pmx>() else {
        return false;
    };
    let node_of = |index: usize| pmx.bones.get(index).map(|bone| bone.node);
    let appended: Vec<NodeId> = pmx
        .bones
        .iter()
        .filter(|bone| moving.contains(&bone.node))
        .filter_map(|bone| node_of(bone.append?.parent))
        .collect();
    moving.extend(appended);
    let reaching: Vec<NodeId> = pmx
        .bones
        .iter()
        .filter(|bone| {
            bone.ik.as_ref().is_some_and(|ik| {
                ik.links
                    .iter()
                    .any(|link| node_of(link.bone).is_some_and(|node| moving.contains(&node)))
            })
        })
        .map(|bone| bone.node)
        .collect();
    let reached = !reaching.is_empty();
    for node in reaching {
        let mut next = Some(node);
        while let Some(node) = next.filter(|node| moving.insert(*node)) {
            next = model.node(node).parent;
        }
    }
    reached
}
