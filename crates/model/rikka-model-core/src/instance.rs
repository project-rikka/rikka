use std::any::Any;
use std::sync::Arc;

use glam::Mat4;

use crate::animation::AnimationId;
use crate::bounds::Bounds;
use crate::material::{Material, MaterialId};
use crate::model::Model;
use crate::node::NodeId;
use crate::player::AnimationPlayer;
use crate::pose::Pose;
use crate::rig::{Phase, PoseState, Rig};
use crate::skin::SkinId;
use crate::tints::Tints;
use crate::transform::Transform;

#[derive(Debug)]
pub struct Instance {
    model: Arc<Model>,
    transform: Mat4,
    pose: Pose,
    player: AnimationPlayer,
    order: Vec<NodeId>,
    locals: Vec<Transform>,
    world: Vec<Mat4>,
    weights: Vec<Vec<f32>>,
    joints: Vec<Vec<Mat4>>,
    rig: Option<Box<dyn Rig>>,
}

impl Clone for Instance {
    fn clone(&self) -> Self {
        Self {
            model: Arc::clone(&self.model),
            transform: self.transform,
            pose: self.pose.clone(),
            player: self.player,
            order: self.order.clone(),
            locals: self.locals.clone(),
            world: self.world.clone(),
            weights: self.weights.clone(),
            joints: self.joints.clone(),
            rig: self.rig.as_ref().map(|rig| rig.duplicate()),
        }
    }
}

impl Instance {
    #[must_use]
    pub fn new(model: Arc<Model>) -> Self {
        let mut instance = Self {
            transform: Mat4::IDENTITY,
            pose: Pose::rest(&model),
            player: AnimationPlayer::new(),
            order: model.order(),
            locals: model.nodes.iter().map(|node| node.transform).collect(),
            world: vec![Mat4::IDENTITY; model.nodes.len()],
            weights: model
                .nodes
                .iter()
                .map(|node| node.morph_weights.clone())
                .collect(),
            joints: model
                .skins
                .iter()
                .map(|skin| vec![Mat4::IDENTITY; skin.joints.len()])
                .collect(),
            rig: model.extensions.rig(&model),
            model,
        };
        instance.refresh();
        instance
    }

    #[must_use]
    pub fn rig_as<R: Rig>(&self) -> Option<&R> {
        let rig: &dyn Any = self.rig.as_deref()?;
        rig.downcast_ref()
    }

    pub fn rig_as_mut<R: Rig>(&mut self) -> Option<&mut R> {
        let rig: &mut dyn Any = self.rig.as_deref_mut()?;
        rig.downcast_mut()
    }

    #[must_use]
    pub fn model(&self) -> &Arc<Model> {
        &self.model
    }

    #[must_use]
    pub fn transform(&self) -> Mat4 {
        self.transform
    }

    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    #[must_use]
    pub fn pose(&self) -> &Pose {
        &self.pose
    }

    pub fn pose_mut(&mut self) -> &mut Pose {
        &mut self.pose
    }

    #[must_use]
    pub fn player(&self) -> &AnimationPlayer {
        &self.player
    }

    pub fn player_mut(&mut self) -> &mut AnimationPlayer {
        &mut self.player
    }

    pub fn play(&mut self, animation: AnimationId) {
        self.player.play(&self.model, animation);
    }

    pub fn advance(&mut self, delta: f32) {
        self.player.advance(delta);
        self.animate();
        self.refresh();
    }

    pub fn seek(&mut self, time: f32) {
        self.player.seek(time);
        self.animate();
        self.refresh();
    }

    pub fn refresh(&mut self) {
        let model = Arc::clone(&self.model);
        gather(&model, &self.pose, &mut self.locals, &mut self.weights);
        self.world.resize(model.nodes.len(), Mat4::IDENTITY);
        for phase in [Phase::BeforePhysics, Phase::AfterPhysics] {
            evaluate_phase(
                &model,
                self.transform,
                self.rig.as_deref_mut(),
                PoseState {
                    order: &self.order,
                    locals: &mut self.locals,
                    world: &mut self.world,
                    weights: &mut self.weights,
                },
                phase,
            );
        }
        self.joints.resize_with(model.skins.len(), Vec::new);
        for (palette, skin) in self.joints.iter_mut().zip(&model.skins) {
            palette.clear();
            palette.extend(
                skin.joints
                    .iter()
                    .zip(&skin.inverse_bind)
                    .map(|(joint, bind)| self.world[joint.index()] * *bind),
            );
        }
    }

    fn animate(&mut self) {
        if self.player.animation().is_some() {
            let model = Arc::clone(&self.model);
            self.pose.reset(&model);
            self.player.apply(&model, &mut self.pose);
        }
    }

    #[must_use]
    pub fn world(&self, node: NodeId) -> Mat4 {
        self.world
            .get(node.index())
            .copied()
            .unwrap_or(Mat4::IDENTITY)
    }

    #[must_use]
    pub fn worlds(&self) -> &[Mat4] {
        &self.world
    }

    #[must_use]
    pub fn joints(&self, skin: SkinId) -> &[Mat4] {
        self.joints.get(skin.index()).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn morph_weights(&self, node: NodeId) -> &[f32] {
        self.weights.get(node.index()).map_or(&[], Vec::as_slice)
    }

    pub fn material(&self, id: MaterialId) -> &Material {
        self.tints()
            .and_then(|tints| tints.get(id))
            .unwrap_or_else(|| self.model.material(id))
    }

    #[must_use]
    pub fn tints(&self) -> Option<&Tints> {
        self.rig.as_deref()?.tints()
    }

    #[must_use]
    pub fn bounds(&self) -> Bounds {
        crate::model::bounds_of(&self.model, |node| self.transform * self.world(node))
    }
}

fn gather(model: &Model, pose: &Pose, locals: &mut Vec<Transform>, weights: &mut Vec<Vec<f32>>) {
    locals.resize(model.nodes.len(), Transform::IDENTITY);
    weights.resize_with(model.nodes.len(), Vec::new);
    for (index, node) in model.nodes.iter().enumerate() {
        let id = NodeId(index as u32);
        locals[index] = pose.local(id);
        let node_weights = &mut weights[index];
        node_weights.clear();
        node_weights.extend_from_slice(pose.weights(id));
        if node_weights.is_empty() {
            node_weights.extend_from_slice(&node.morph_weights);
        }
        let targets = node
            .mesh
            .map_or(0, |mesh| model.mesh(mesh).morph_targets.len());
        node_weights.resize(targets.max(node_weights.len()), 0.0);
    }
}

fn evaluate_phase(
    model: &Model,
    transform: Mat4,
    rig: Option<&mut (dyn Rig + 'static)>,
    state: PoseState<'_>,
    phase: Phase,
) {
    if let Some(rig) = rig {
        rig.evaluate(model, transform, state, phase);
        return;
    }
    if phase == Phase::AfterPhysics {
        return;
    }
    model.place_all(state.order, state.locals, state.world);
}
