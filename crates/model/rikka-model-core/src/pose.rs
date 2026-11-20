use crate::model::Model;
use crate::node::NodeId;
use crate::transform::Transform;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pose {
    locals: Vec<Transform>,
    weights: Vec<Vec<f32>>,
}

impl Pose {
    #[must_use]
    pub fn rest(model: &Model) -> Self {
        Self {
            locals: model.nodes.iter().map(|node| node.transform).collect(),
            weights: model
                .nodes
                .iter()
                .map(|node| node.morph_weights.clone())
                .collect(),
        }
    }

    pub fn reset(&mut self, model: &Model) {
        for (slot, node) in self.locals.iter_mut().zip(&model.nodes) {
            *slot = node.transform;
        }
        for (slot, node) in self.weights.iter_mut().zip(&model.nodes) {
            slot.clear();
            slot.extend_from_slice(&node.morph_weights);
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.locals.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.locals.is_empty()
    }

    #[must_use]
    pub fn local(&self, node: NodeId) -> Transform {
        self.locals
            .get(node.index())
            .copied()
            .unwrap_or(Transform::IDENTITY)
    }

    pub fn set_local(&mut self, node: NodeId, transform: Transform) {
        if let Some(slot) = self.locals.get_mut(node.index()) {
            *slot = transform;
        }
    }

    pub fn local_mut(&mut self, node: NodeId) -> Option<&mut Transform> {
        self.locals.get_mut(node.index())
    }

    #[must_use]
    pub fn weights(&self, node: NodeId) -> &[f32] {
        self.weights.get(node.index()).map_or(&[], Vec::as_slice)
    }

    pub fn weights_mut(&mut self, node: NodeId) -> Option<&mut Vec<f32>> {
        self.weights.get_mut(node.index())
    }

    pub fn blend(&mut self, other: &Pose, factor: f32) {
        let factor = factor.clamp(0.0, 1.0);
        for (slot, &theirs) in self.locals.iter_mut().zip(&other.locals) {
            *slot = slot.lerp(theirs, factor);
        }
        for (slot, theirs) in self.weights.iter_mut().zip(&other.weights) {
            for (weight, &other) in slot.iter_mut().zip(theirs) {
                *weight += (other - *weight) * factor;
            }
        }
    }
}
