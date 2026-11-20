use rikka_model_core::glam::{Mat4, Quat, Vec3};
use rikka_model_core::{HumanBone, Model, NodeId};

use crate::mapping::Mapping;
use crate::skeleton::{position_of, rotation_of, tip};
use crate::worlds::{Joint, Worlds};

const SHORTEST: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lobe {
    pub node: NodeId,
    pub anchor: HumanBone,
    pub root: Joint,
    pub parent: Quat,
    pub tip: Vec3,
}

pub(crate) fn lobes(
    model: &Model,
    mapping: &Mapping,
    folded: &[Option<HumanBone>],
    rest: &[Mat4],
    still: &Worlds,
    scale: f32,
) -> Vec<Lobe> {
    mapping
        .flesh_swings()
        .iter()
        .filter_map(|swing| {
            let node = swing.root();
            let anchor = folded.get(swing.anchor()?.index()).copied().flatten()?;
            let joint = still.get(anchor)?;
            let world = rest.get(node.index())?;
            let origin = position_of(*world);
            let turn = rotation_of(*world);
            let far = |point: &Vec3| point.distance(origin) > SHORTEST * scale;
            let tip = descendants(model, node)
                .filter_map(|node| rest.get(node.index()).map(|matrix| position_of(*matrix)))
                .filter(far)
                .max_by(|one, other| one.distance(origin).total_cmp(&other.distance(origin)))
                .or_else(|| tip(model, node, rest).map(|offset| origin + offset))
                .filter(far)?;
            let parent = model
                .node(node)
                .parent
                .and_then(|parent| rest.get(parent.index()))
                .map_or(joint.rotation, |matrix| rotation_of(*matrix));
            Some(Lobe {
                node,
                anchor,
                root: Joint {
                    position: joint.local(origin),
                    rotation: joint.rotation.inverse() * turn,
                },
                parent: joint.rotation.inverse() * parent,
                tip: turn.inverse() * (tip - origin),
            })
        })
        .collect()
}

fn descendants(model: &Model, root: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    let mut waiting = model.node(root).children.clone();
    std::iter::from_fn(move || {
        let node = waiting.pop()?;
        waiting.extend(model.node(node).children.iter().copied());
        Some(node)
    })
}
