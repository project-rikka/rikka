use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use rikka_gpu::ComputeBuffer;
use rikka_model_core::{Instance, Model, NodeId};
use rikka_physics_tailor::{Grab, Wind};

use crate::figure::FigureId;

#[must_use]
#[derive(Clone, Debug)]
pub struct Inputs {
    pub(crate) figures: Vec<(FigureId, Posture)>,
    pub(crate) wind: Wind,
    pub(crate) reactions: Vec<(FigureId, NodeId, ComputeBuffer)>,
    pub(crate) grabs: Vec<(FigureId, Grab)>,
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs {
            figures: Vec::new(),
            wind: Wind::CALM,
            reactions: Vec::new(),
            grabs: Vec::new(),
        }
    }
}

impl Inputs {
    pub fn new() -> Inputs {
        Inputs::default()
    }

    pub fn figure(mut self, figure: FigureId, instance: &Instance) -> Inputs {
        self.figures.retain(|(id, _)| *id != figure);
        self.figures.push((
            figure,
            Posture {
                model: Arc::clone(instance.model()),
                transform: instance.transform(),
                worlds: instance.worlds().to_vec(),
            },
        ));
        self
    }

    pub fn reaction(mut self, figure: FigureId, node: NodeId, buffer: &ComputeBuffer) -> Inputs {
        self.reactions
            .retain(|(id, drawn, _)| (*id, *drawn) != (figure, node));
        self.reactions.push((figure, node, buffer.clone()));
        self
    }

    pub fn wind(mut self, wind: Wind) -> Inputs {
        self.wind = wind;
        self
    }

    pub fn grab(mut self, figure: FigureId, grab: Grab) -> Inputs {
        self.grabs
            .retain(|(id, gripped)| (*id, gripped.id) != (figure, grab.id));
        self.grabs.push((figure, grab));
        self
    }

    pub(crate) fn posture(&self, figure: FigureId) -> Option<&Posture> {
        self.figures
            .iter()
            .find(|(id, _)| *id == figure)
            .map(|(_, posture)| posture)
    }

    pub(crate) fn grabs_of(&self, figure: FigureId) -> Vec<&Grab> {
        self.grabs
            .iter()
            .filter(|(id, _)| *id == figure)
            .map(|(_, grab)| grab)
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Posture {
    pub(crate) model: Arc<Model>,
    pub(crate) transform: Mat4,
    pub(crate) worlds: Vec<Mat4>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Joint {
    pub(crate) scale: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) translation: Vec3,
}

impl Joint {
    pub(crate) fn from_matrix(matrix: Mat4) -> Joint {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Joint {
            scale,
            rotation: rotation.normalize(),
            translation,
        }
    }

    pub(crate) fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub(crate) fn lerp(&self, other: &Joint, t: f32) -> Joint {
        Joint {
            scale: self.scale.lerp(other.scale, t),
            rotation: self.rotation.slerp(other.rotation, t).normalize(),
            translation: self.translation.lerp(other.translation, t),
        }
    }

    pub(crate) fn point(&self, local: Vec3) -> Vec3 {
        self.translation + self.rotation * (self.scale * local)
    }
}
