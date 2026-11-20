use glam::{Mat4, Vec3};
use rikka_humanoid::Anatomy;
use rikka_model_core::{HumanBone, Model, NodeId};

const CORE: [HumanBone; 13] = [
    HumanBone::Hips,
    HumanBone::Spine,
    HumanBone::Chest,
    HumanBone::UpperChest,
    HumanBone::Neck,
    HumanBone::Head,
    HumanBone::LeftShoulder,
    HumanBone::RightShoulder,
    HumanBone::LeftUpperArm,
    HumanBone::RightUpperArm,
    HumanBone::LeftUpperLeg,
    HumanBone::RightUpperLeg,
    HumanBone::Jaw,
];

#[derive(Clone, Debug)]
pub(crate) struct Skeleton {
    pub(crate) parents: Vec<Option<NodeId>>,
    pub(crate) origins: Vec<Vec3>,
    pub(crate) core: Vec<bool>,
    pub(crate) under_head: Vec<bool>,
    pub(crate) order: Vec<NodeId>,
}

impl Skeleton {
    pub(crate) fn new(model: &Model, rest_worlds: &[Mat4], anatomy: &Anatomy) -> Skeleton {
        let count = model.nodes.len();
        let order = model.order();
        let parents: Vec<Option<NodeId>> = model.nodes.iter().map(|node| node.parent).collect();
        let core: Vec<bool> = (0..count)
            .map(|index| {
                anatomy
                    .region(NodeId(index as u32))
                    .is_some_and(|bone| CORE.contains(&bone))
            })
            .collect();
        let mut under_head = vec![false; count];
        if let Some(head) = anatomy.node(HumanBone::Head) {
            under_head[head.index()] = true;
        }
        let mut skeleton = Skeleton {
            parents,
            origins: rest_worlds
                .iter()
                .map(|world| world.w_axis.truncate())
                .collect(),
            core,
            under_head: Vec::new(),
            order,
        };
        skeleton.under_head = skeleton.descendants_of(&under_head);
        skeleton
    }

    pub(crate) fn descendants_of(&self, marked: &[bool]) -> Vec<bool> {
        let mut carried = marked.to_vec();
        carried.resize(self.parents.len(), false);
        for node in &self.order {
            if let Some(parent) = self.parents[node.index()]
                && carried[parent.index()]
            {
                carried[node.index()] = true;
            }
        }
        carried
    }
}
