use rikka_humanoid::{Anatomy, Swing};
use rikka_mmd::{Pmx, PmxBoneFlags};
use rikka_model_core::{HumanBone, Humanoid, Model, NodeId};
use rikka_vrm::Vrm;

use crate::error::MappingError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Append {
    pub(crate) source: NodeId,
    pub(crate) ratio: f32,
    pub(crate) rotates: bool,
    pub(crate) moves: bool,
}

#[derive(Clone, Debug)]
pub struct Mapping {
    humanoid: Humanoid,
    carrier: NodeId,
    intermediates: Vec<NodeId>,
    twists: Vec<(HumanBone, NodeId)>,
    followers: Vec<NodeId>,
    appends: Vec<Option<Append>>,
    reaching: Vec<usize>,
    driven: Vec<NodeId>,
    marked: Vec<bool>,
    flesh_swings: Vec<Swing>,
}

impl Mapping {
    pub fn from_anatomy(model: &Model, anatomy: &Anatomy) -> Result<Self, MappingError> {
        let humanoid = anatomy.humanoid().clone();
        let missing: Vec<HumanBone> = humanoid.missing().collect();
        let Some(hips) = humanoid
            .bone(HumanBone::Hips)
            .filter(|_| missing.is_empty())
        else {
            return Err(MappingError { missing });
        };
        let twists: Vec<(HumanBone, NodeId)> = anatomy.twists().collect();
        let mut mapping = match model.extensions.get::<Pmx>() {
            Some(pmx) if !model.extensions.contains::<Vrm>() => {
                Self::from_pmx(model, pmx, humanoid, hips, twists)
            }
            _ => Self::settled(
                model,
                humanoid,
                hips,
                twists,
                vec![None; model.nodes.len()],
                Vec::new(),
            ),
        };
        mapping.flesh_swings = anatomy
            .swings()
            .iter()
            .filter(|swing| swing.is_flesh())
            .cloned()
            .collect();
        Ok(mapping)
    }

    pub(crate) fn flesh_swings(&self) -> &[Swing] {
        &self.flesh_swings
    }

    fn from_pmx(
        model: &Model,
        pmx: &Pmx,
        humanoid: Humanoid,
        hips: NodeId,
        twists: Vec<(HumanBone, NodeId)>,
    ) -> Self {
        let bone_of = |node: NodeId| pmx.bones.iter().position(|bone| bone.node == node);
        let mut carrier = hips;
        for _ in 0..pmx.bones.len() {
            let above = bone_of(carrier)
                .and_then(|bone| pmx.bones[bone].parent)
                .and_then(|parent| pmx.bones.get(parent));
            match above {
                Some(parent) => carrier = parent.node,
                None => break,
            }
        }

        let mut appends = vec![None; model.nodes.len()];
        for bone in &pmx.bones {
            let Some(append) = bone.append.filter(|append| append.parent < pmx.bones.len()) else {
                continue;
            };
            let rotates = bone.flags.has(PmxBoneFlags::APPEND_ROTATE);
            let moves = bone.flags.has(PmxBoneFlags::APPEND_TRANSLATE);
            if (rotates || moves) && bone.node.index() < appends.len() {
                appends[bone.node.index()] = Some(Append {
                    source: pmx.bones[append.parent].node,
                    ratio: append.ratio,
                    rotates,
                    moves,
                });
            }
        }

        let mut mapping = Self::settled(model, humanoid, carrier, twists, appends, Vec::new());
        let moved: Vec<NodeId> = mapping
            .human_nodes()
            .chain(mapping.twists.iter().map(|(_, node)| *node))
            .collect();
        let mut reached = Vec::new();
        for (index, bone) in pmx.bones.iter().enumerate() {
            let Some(ik) = &bone.ik else {
                continue;
            };
            let links: Vec<NodeId> = ik
                .links
                .iter()
                .filter(|link| link.bone < pmx.bones.len())
                .map(|link| pmx.bones[link.bone].node)
                .collect();
            if !links.iter().any(|link| moved.contains(link)) {
                continue;
            }
            mapping.reaching.push(index);
            reached.push(bone.node);
            reached.extend(links);
            if let Some(target) = pmx.bones.get(ik.target) {
                reached.push(target.node);
            }
        }
        mapping.mark(reached);
        mapping
    }

    fn settled(
        model: &Model,
        humanoid: Humanoid,
        carrier: NodeId,
        twists: Vec<(HumanBone, NodeId)>,
        appends: Vec<Option<Append>>,
        reaching: Vec<usize>,
    ) -> Self {
        let mut mapping = Self {
            humanoid,
            carrier,
            intermediates: Vec::new(),
            twists,
            followers: Vec::new(),
            appends,
            reaching,
            driven: Vec::new(),
            marked: vec![false; model.nodes.len()],
            flesh_swings: Vec::new(),
        };
        mapping.classify(model);
        let posed_nodes: Vec<NodeId> = std::iter::once(mapping.carrier)
            .chain(mapping.intermediates.iter().copied())
            .chain(mapping.human_nodes())
            .chain(mapping.twists.iter().map(|(_, node)| *node))
            .chain(mapping.followers.iter().copied())
            .collect();
        mapping.mark(posed_nodes);
        mapping
    }

    fn classify(&mut self, model: &Model) {
        let is_posed = |mapping: &Self, node: NodeId| {
            node == mapping.carrier
                || mapping.bone_of(node).is_some()
                || mapping
                    .twists
                    .iter()
                    .any(|(_, twist_node)| *twist_node == node)
        };
        let starts: Vec<NodeId> = self
            .human_nodes()
            .chain(self.twists.iter().map(|(_, node)| *node))
            .filter(|node| *node != self.carrier)
            .collect();
        let mut between = Vec::new();
        let mut trailing = Vec::new();
        for start in &starts {
            let mut path = Vec::new();
            let mut node = model.node(*start).parent;
            let mut reached = false;
            while let Some(ancestor) = node {
                if is_posed(self, ancestor) {
                    reached = true;
                    break;
                }
                if path.contains(&ancestor) || path.len() > model.nodes.len() {
                    break;
                }
                path.push(ancestor);
                node = model.node(ancestor).parent;
            }
            if !reached {
                continue;
            }
            for ancestor in path {
                match self.append(ancestor) {
                    Some(_) => trailing.push(ancestor),
                    None => between.push(ancestor),
                }
            }
        }
        for node in starts.iter().chain(&trailing) {
            if let Some(append) = self.append(*node)
                && !is_posed(self, append.source)
                && self.append(append.source).is_none()
            {
                between.push(append.source);
            }
        }
        between.sort_unstable();
        between.dedup();
        self.intermediates = between;

        loop {
            let before = trailing.len();
            for (index, append) in self.appends.iter().enumerate() {
                let node = NodeId(index as u32);
                let Some(append) = append else {
                    continue;
                };
                let led = is_posed(self, append.source)
                    || self.intermediates.contains(&append.source)
                    || trailing.contains(&append.source);
                if led && !is_posed(self, node) && !trailing.contains(&node) {
                    trailing.push(node);
                }
            }
            if trailing.len() == before {
                break;
            }
        }
        trailing.sort_unstable();
        trailing.dedup();
        self.followers = trailing;
    }

    fn mark(&mut self, nodes: Vec<NodeId>) {
        for node in nodes {
            if let Some(slot) = self.marked.get_mut(node.index()) {
                *slot = true;
            }
        }
        self.driven = self
            .marked
            .iter()
            .enumerate()
            .filter(|(_, marked)| **marked)
            .map(|(index, _)| NodeId(index as u32))
            .collect();
    }

    #[must_use]
    pub fn node(&self, bone: HumanBone) -> Option<NodeId> {
        self.humanoid.bone(bone)
    }

    #[must_use]
    pub fn bone_of(&self, node: NodeId) -> Option<HumanBone> {
        self.humanoid
            .bones()
            .find(|(_, twist_node)| *twist_node == node)
            .map(|(bone, _)| bone)
    }

    fn human_nodes(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.humanoid.bones().map(|(_, node)| node)
    }

    #[must_use]
    pub fn carrier(&self) -> NodeId {
        self.carrier
    }

    #[must_use]
    pub fn intermediates(&self) -> &[NodeId] {
        &self.intermediates
    }

    #[must_use]
    pub fn twist(&self, bone: HumanBone) -> Option<NodeId> {
        self.twists
            .iter()
            .find(|(mapped, _)| *mapped == bone)
            .map(|(_, node)| *node)
    }

    pub fn twists(&self) -> impl Iterator<Item = (HumanBone, NodeId)> + '_ {
        self.twists.iter().copied()
    }

    #[must_use]
    pub fn is_driven(&self, node: NodeId) -> bool {
        self.marked.get(node.index()).copied().unwrap_or(false)
    }

    pub fn driven(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.driven.iter().copied()
    }

    #[must_use]
    pub fn reaching(&self) -> &[usize] {
        &self.reaching
    }

    pub(crate) fn append(&self, node: NodeId) -> Option<Append> {
        self.appends.get(node.index()).copied().flatten()
    }

    #[must_use]
    pub fn folded(&self, model: &Model) -> Vec<Option<HumanBone>> {
        (0..model.nodes.len())
            .map(|index| self.fold(model, NodeId(index as u32)))
            .collect()
    }

    fn fold(&self, model: &Model, node: NodeId) -> Option<HumanBone> {
        let mut current = Some(node);
        for _ in 0..=model.nodes.len() {
            let node = current?;
            if let Some(bone) = self.bone_of(node) {
                return Some(bone);
            }
            if let Some((bone, _)) = self.twists.iter().find(|(_, twist)| *twist == node) {
                return Some(*bone);
            }
            current = match self.append(node) {
                Some(append) if append.rotates && append.ratio > 0.0 => Some(append.source),
                _ => model.node(node).parent,
            };
        }
        None
    }
}
