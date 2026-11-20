use std::collections::{BTreeSet, HashMap, HashSet};

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
        let mut frames = HashMap::new();
        for part in Part::ALL {
            let bones = bones_of(part);
            if bones.is_empty() {
                continue;
            }
            let mut moving = moved_by(model, body, &bones);
            if reached_through(model, &mut moving) {
                moving.extend(root.iter().copied());
            }
            let keyed: BTreeSet<u32> = keyed
                .iter()
                .filter(|(node, _)| moving.contains(node))
                .map(|(_, frame)| *frame)
                .collect();
            frames.insert(part, keyed.into_iter().collect());
        }
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
