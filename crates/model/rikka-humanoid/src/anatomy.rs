use rikka_model_core::glam::Vec3;
use rikka_model_core::{HumanBone, Humanoid, Model, NodeId};

use crate::frame::Frame;
use crate::naming::named;
use crate::parts::{parts, regions};
use crate::recognize::recognize;
use crate::swing::{Swing, swings};

const TWISTED: [(HumanBone, HumanBone); 4] = [
    (HumanBone::LeftUpperArm, HumanBone::LeftLowerArm),
    (HumanBone::LeftLowerArm, HumanBone::LeftHand),
    (HumanBone::RightUpperArm, HumanBone::RightLowerArm),
    (HumanBone::RightLowerArm, HumanBone::RightHand),
];

#[derive(Clone, Debug)]
pub struct Anatomy {
    humanoid: Humanoid,
    twists: Vec<(HumanBone, NodeId)>,
    parts: Vec<Option<HumanBone>>,
    regions: Vec<Option<HumanBone>>,
    swings: Vec<Swing>,
    hung: Vec<Option<usize>>,
    forward: Vec3,
}

impl Anatomy {
    #[must_use]
    pub fn recognize(model: &Model) -> Anatomy {
        let evidence = model.rig_facts();
        let frame = Frame::build(model, &evidence);
        let named = match evidence.humanoid {
            Some(_) => None,
            None => named(model, &frame),
        };
        let recognized = recognize(&frame, &evidence, named);
        let humanoid = recognized.humanoid;
        let parts = parts(&frame, &humanoid);
        let regions = regions(&frame, &parts);
        let (swings, hung) = swings(&frame, &regions, &recognized.facing);
        Anatomy {
            twists: twists(&frame, &humanoid),
            humanoid,
            parts,
            regions,
            swings,
            hung,
            forward: recognized.facing.forward,
        }
    }

    #[must_use]
    pub fn humanoid(&self) -> &Humanoid {
        &self.humanoid
    }

    #[must_use]
    pub fn node(&self, bone: HumanBone) -> Option<NodeId> {
        self.humanoid.bone(bone)
    }

    #[must_use]
    pub fn twist(&self, bone: HumanBone) -> Option<NodeId> {
        self.twists
            .iter()
            .find(|(twisted_bone, _)| *twisted_bone == bone)
            .map(|(_, node)| *node)
    }

    pub fn twists(&self) -> impl Iterator<Item = (HumanBone, NodeId)> + '_ {
        self.twists.iter().copied()
    }

    #[must_use]
    pub fn part(&self, node: NodeId) -> Option<HumanBone> {
        self.parts.get(node.index()).copied().flatten()
    }

    #[must_use]
    pub fn region(&self, node: NodeId) -> Option<HumanBone> {
        self.regions.get(node.index()).copied().flatten()
    }

    #[must_use]
    pub fn swings(&self) -> &[Swing] {
        &self.swings
    }

    #[must_use]
    pub fn swing(&self, node: NodeId) -> Option<&Swing> {
        self.hung
            .get(node.index())
            .copied()
            .flatten()
            .map(|index| &self.swings[index])
    }

    #[must_use]
    pub fn forward(&self) -> Vec3 {
        self.forward
    }
}

fn twists(frame: &Frame, humanoid: &Humanoid) -> Vec<(HumanBone, NodeId)> {
    TWISTED
        .iter()
        .filter_map(|(upper, lower)| {
            let (from, to) = (humanoid.bone(*upper)?, humanoid.bone(*lower)?);
            let path = frame.path(from, to);
            let between = &path[..path.len().saturating_sub(1)];
            let twist = between
                .iter()
                .copied()
                .find(|node| frame.is_twisting(*node) && !frame.is_follower(*node))
                .or_else(|| {
                    between
                        .iter()
                        .copied()
                        .find(|node| !frame.is_follower(*node))
                })?;
            Some((*upper, twist))
        })
        .collect()
}
