use std::collections::VecDeque;

use rikka_model_core::glam::Vec3;
use rikka_model_core::{Follow, Model, NodeId};

use rikka_model_core::RigFacts;

use crate::lineage::Lineage;

const SAME: f32 = 0.002;
const WHOLE: f32 = 0.999;

#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pub(crate) origins: Vec<Vec3>,
    pub(crate) parents: Vec<Option<NodeId>>,
    pub(crate) order: Vec<NodeId>,
    pub(crate) swinging: Vec<bool>,
    alias: Vec<NodeId>,
    follows: Vec<Option<Follow>>,
    twisting: Vec<bool>,
    flesh: Vec<f32>,
    subtree_flesh: Vec<f32>,
    lineage: Lineage,
    below: Vec<Vec<NodeId>>,
    weighted: Vec<bool>,
    pub(crate) span: f32,
    pub(crate) centre: Vec3,
}

impl Frame {
    pub(crate) fn build(model: &Model, evidence: &RigFacts) -> Frame {
        let count = model.nodes.len();
        let origins: Vec<Vec3> = model
            .world_transforms()
            .iter()
            .map(|world| world.w_axis.truncate())
            .collect();
        let parents: Vec<Option<NodeId>> = model
            .nodes
            .iter()
            .map(|node| node.parent.filter(|parent| parent.index() < count))
            .collect();
        let order = model.order();
        let mut swinging = vec![false; count];
        for node in &evidence.swinging {
            swinging[node.index()] = true;
        }
        let mut moved = swinging.clone();
        for node in &order {
            if let Some(parent) = parents[node.index()]
                && moved[parent.index()]
            {
                moved[node.index()] = true;
            }
        }
        let mut follows: Vec<Option<Follow>> = vec![None; count];
        let mut twisting = vec![false; count];
        let mut source: Vec<Option<NodeId>> = vec![None; count];
        for follow in &evidence.follows {
            follows[follow.node.index()] = Some(*follow);
            let together =
                origins[follow.node.index()].distance(origins[follow.source.index()]) <= SAME;
            if follow.ratio >= WHOLE && together {
                source[follow.node.index()] = Some(follow.source);
            }
        }
        for node in &evidence.twisting {
            twisting[node.index()] = true;
        }
        let alias: Vec<NodeId> = (0..count)
            .map(|index| {
                let mut node = NodeId(index as u32);
                for _ in 0..count {
                    match source[node.index()] {
                        Some(next) if next.index() != index => node = next,
                        _ => break,
                    }
                }
                node
            })
            .collect();
        let mut flesh = vec![0.0f32; count];
        for (index, node) in model.nodes.iter().enumerate() {
            let Some(mesh) = node.mesh.map(|mesh| model.mesh(mesh)) else {
                continue;
            };
            let skin = node
                .skin
                .filter(|_| mesh.vertices.is_skinned())
                .map(|skin| model.skin(skin));
            match skin {
                Some(skin) => {
                    for vertex in 0..mesh.vertices.len() {
                        for (joint, weight) in
                            mesh.vertices.influences(vertex).into_iter().flatten()
                        {
                            if let Some(joint) = skin.joints.get(usize::from(joint))
                                && joint.index() < count
                            {
                                flesh[alias[joint.index()].index()] += weight;
                            }
                        }
                    }
                }
                None => flesh[alias[index].index()] += mesh.vertices.len() as f32,
            }
        }
        let standing = |index: usize| alias[index].index() == index && !moved[index];
        let mut above: Vec<Option<NodeId>> = (0..count)
            .map(|index| {
                let parent = parents[index]?;
                let lifted = alias[parent.index()];
                Some(match lifted.index() == index {
                    true => parent,
                    false => lifted,
                })
                .filter(|parent| standing(parent.index()))
            })
            .collect();
        let mut below: Vec<Vec<NodeId>> = vec![Vec::new(); count];
        for (index, parent) in above.iter().enumerate() {
            if standing(index)
                && let Some(parent) = parent
            {
                below[parent.index()].push(NodeId(index as u32));
            }
        }
        let mut depth = vec![u32::MAX; count];
        let mut reached = Vec::with_capacity(count);
        let mut queue: VecDeque<NodeId> = (0..count)
            .filter(|index| standing(*index) && above[*index].is_none())
            .map(|index| NodeId(index as u32))
            .collect();
        for node in &queue {
            depth[node.index()] = 0;
        }
        while let Some(node) = queue.pop_front() {
            reached.push(node);
            for child in &below[node.index()] {
                if depth[child.index()] == u32::MAX {
                    depth[child.index()] = depth[node.index()] + 1;
                    queue.push_back(*child);
                }
            }
        }
        for (index, parent) in above.iter_mut().enumerate() {
            if depth[index] == u32::MAX {
                *parent = None;
            }
        }
        let mut subtree_flesh = flesh.clone();
        for node in reached.iter().rev() {
            if let Some(parent) = above[node.index()] {
                subtree_flesh[parent.index()] += subtree_flesh[node.index()];
            }
        }
        let mut weighted = vec![false; count];
        for node in &reached {
            weighted[node.index()] = subtree_flesh[node.index()] > 0.0;
        }
        for children in &mut below {
            children.retain(|child| weighted[child.index()]);
        }
        let standing_origins: Vec<Vec3> = reached
            .iter()
            .filter(|node| weighted[node.index()])
            .map(|node| origins[node.index()])
            .collect();
        let (low, high) = standing_origins.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(low, high), origin| (low.min(*origin), high.max(*origin)),
        );
        let span = match standing_origins.is_empty() {
            true => 1.0,
            false => (high.y - low.y).max(1e-3),
        };
        let centre = match standing_origins.is_empty() {
            true => Vec3::ZERO,
            false => standing_origins.iter().copied().sum::<Vec3>() / standing_origins.len() as f32,
        };
        Frame {
            origins,
            parents,
            order,
            swinging,
            alias,
            follows,
            twisting,
            flesh,
            subtree_flesh,
            lineage: Lineage::new(above, depth),
            below,
            weighted,
            span,
            centre,
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.origins.len()
    }

    pub(crate) fn origin(&self, node: NodeId) -> Vec3 {
        self.origins[node.index()]
    }

    pub(crate) fn alias(&self, node: NodeId) -> NodeId {
        self.alias[node.index()]
    }

    pub(crate) fn is_weighted(&self, node: NodeId) -> bool {
        self.weighted[node.index()]
    }

    pub(crate) fn is_follower(&self, node: NodeId) -> bool {
        self.follows[node.index()].is_some()
    }

    pub(crate) fn follow(&self, node: NodeId) -> Option<Follow> {
        self.follows[node.index()]
    }

    pub(crate) fn is_twisting(&self, node: NodeId) -> bool {
        self.twisting[node.index()]
    }

    pub(crate) fn is_joint(&self, node: NodeId) -> bool {
        !self.is_follower(node) && !self.is_twisting(node)
    }

    pub(crate) fn flesh(&self, node: NodeId) -> f32 {
        self.flesh[node.index()]
    }

    pub(crate) fn subtree_flesh(&self, node: NodeId) -> f32 {
        self.subtree_flesh[node.index()]
    }

    pub(crate) fn weighted(&self) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.count())
            .filter(|index| self.weighted[*index])
            .map(|index| NodeId(index as u32))
    }

    pub(crate) fn children(&self, node: NodeId) -> &[NodeId] {
        &self.below[node.index()]
    }

    pub(crate) fn above(&self, node: NodeId) -> Option<NodeId> {
        self.lineage.above(node)
    }

    pub(crate) fn subtree(&self, node: NodeId) -> Vec<NodeId> {
        let mut nodes = vec![node];
        let mut next = 0;
        while next < nodes.len() {
            nodes.extend_from_slice(self.children(nodes[next]));
            next += 1;
        }
        nodes
    }

    pub(crate) fn contains(&self, ancestor: NodeId, node: NodeId) -> bool {
        self.lineage.contains(ancestor, node)
    }

    pub(crate) fn common(&self, a: NodeId, b: NodeId) -> Option<NodeId> {
        self.lineage.common(a, b)
    }

    pub(crate) fn path(&self, from: NodeId, to: NodeId) -> Vec<NodeId> {
        self.lineage.path(from, to)
    }

    pub(crate) fn joints(&self, path: &[NodeId]) -> Vec<NodeId> {
        let mut joints: Vec<NodeId> = Vec::with_capacity(path.len());
        for node in path.iter().copied().filter(|node| self.is_joint(*node)) {
            match joints.last() {
                Some(last) if self.origin(*last).distance(self.origin(node)) <= SAME => {
                    *joints.last_mut().expect("a joint") = node;
                }
                _ => joints.push(node),
            }
        }
        joints
    }
}
