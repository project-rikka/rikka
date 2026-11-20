use glam::Mat4;
use rikka_gpu::ComputeBuffer;
use rikka_model_core::{Instance, Model, NodeId};

use crate::body::BodyId;
use crate::emission::Emission;

#[must_use]
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub(crate) bodies: Vec<(BodyId, Posture)>,
    pub(crate) drapes: Vec<DrapeInput>,
    pub(crate) emissions: Vec<Emission>,
    pub(crate) drying: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct DrapeInput {
    pub(crate) body: BodyId,
    pub(crate) node: NodeId,
    pub(crate) buffer: ComputeBuffer,
}

impl Inputs {
    pub fn new() -> Inputs {
        Inputs::default()
    }

    pub fn body(self, body: BodyId, instance: &Instance) -> Inputs {
        self.posture(body, Posture::from_instance(instance))
    }

    pub fn posture(mut self, body: BodyId, posture: Posture) -> Inputs {
        self.bodies.retain(|(posed, _)| *posed != body);
        self.bodies.push((body, posture));
        self
    }

    pub fn drape(mut self, body: BodyId, node: NodeId, drape: &ComputeBuffer) -> Inputs {
        self.drapes
            .retain(|drape| drape.body != body || drape.node != node);
        self.drapes.push(DrapeInput {
            body,
            node,
            buffer: drape.clone(),
        });
        self
    }

    pub fn emit(mut self, emission: Emission) -> Inputs {
        self.emissions.push(emission);
        self
    }

    pub fn drying(mut self, per_second: f32) -> Inputs {
        self.drying = per_second.max(0.0);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Posture {
    pub(crate) palette: Vec<Mat4>,
    pub(crate) morph_weights: Vec<f32>,
}

impl Posture {
    #[must_use]
    pub fn from_instance(instance: &Instance) -> Posture {
        let model = instance.model();
        let transform = instance.transform();
        let mut palette = Vec::new();
        let mut morph_weights = Vec::new();
        for (index, node) in model.nodes.iter().enumerate() {
            let Some(mesh) = node.mesh else {
                continue;
            };
            let id = NodeId(index as u32);
            match part_placement(model, instance, id, transform) {
                Placement::Skinned(joints) => palette.extend(joints),
                Placement::Rigid(matrix) => palette.push(matrix),
            }
            let mesh = model.mesh(mesh);
            let weights = instance.morph_weights(id);
            morph_weights.extend(
                (0..mesh.morph_targets.len())
                    .map(|target| weights.get(target).copied().unwrap_or(0.0)),
            );
        }
        Posture {
            palette,
            morph_weights,
        }
    }
}

enum Placement {
    Skinned(Vec<Mat4>),
    Rigid(Mat4),
}

fn part_placement(model: &Model, instance: &Instance, node: NodeId, transform: Mat4) -> Placement {
    let described = model.node(node);
    let skinned = described.skin.filter(|_| {
        described
            .mesh
            .is_some_and(|mesh| model.mesh(mesh).vertices.is_skinned())
    });
    match (described.skin, skinned) {
        (_, Some(skin)) => Placement::Skinned(
            instance
                .joints(skin)
                .iter()
                .map(|joint| transform * *joint)
                .collect(),
        ),
        (Some(_), None) => Placement::Rigid(transform),
        (None, None) => Placement::Rigid(transform * instance.world(node)),
    }
}
