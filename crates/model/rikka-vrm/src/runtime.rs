use rikka_model_core::glam::{EulerRot, Mat4, Quat, Vec3};

use rikka_model_core::{
    HumanBone, Material, Model, NodeId, Phase, PoseState, Rig, Shading, Tints, Transform,
};

use super::animation::Gaze;
use super::avatar::Vrm;
use super::constraint::ConstraintState;
use super::expression::{ExpressionOverride, ExpressionPreset, MaterialColor, Procedural};
use super::look_at::LookAtKind;

#[derive(Clone, Copy, Debug)]
struct Eye {
    node: NodeId,
    parent_rest: Quat,
}

#[derive(Clone, Debug)]
pub struct Avatar {
    weights: Vec<f32>,
    resolved: Vec<f32>,
    look_at: Option<Vec3>,
    gaze: Option<Gaze>,
    tints: Tints,
    tinted: bool,
    order: Vec<(NodeId, Option<usize>)>,
    head_rest: Quat,
    eyes: [Option<Eye>; 2],
}

impl Avatar {
    #[must_use]
    pub fn new(model: &Model) -> Option<Self> {
        let vrm = model.extensions.get::<Vrm>()?;
        let rest: Vec<Quat> = model
            .world_transforms()
            .iter()
            .map(|world| Transform::from_matrix(*world).rotation)
            .collect();
        let resting = |node: Option<NodeId>| {
            node.and_then(|node| rest.get(node.index()).copied())
                .unwrap_or(Quat::IDENTITY)
        };
        let eye = |bone| {
            let node = vrm.bone(bone)?;
            Some(Eye {
                node,
                parent_rest: resting(model.nodes.get(node.index())?.parent),
            })
        };

        let mut constrained = vec![None; model.nodes.len()];
        for (index, node_constraint) in vrm.constraints.iter().enumerate() {
            if let Some(slot) = constrained.get_mut(node_constraint.node.index()) {
                *slot = Some(index);
            }
        }
        Some(Self {
            weights: vec![0.0; vrm.expressions.len()],
            resolved: vec![0.0; vrm.expressions.len()],
            look_at: None,
            gaze: None,
            tinted: false,
            tints: bound_materials(model, vrm),
            order: order(model, vrm, &constrained)
                .into_iter()
                .map(|node| (node, constrained[node.index()]))
                .collect(),
            head_rest: resting(vrm.bone(HumanBone::Head)),
            eyes: [eye(HumanBone::LeftEye), eye(HumanBone::RightEye)],
        })
    }

    #[must_use]
    pub fn weight(&self, expression: usize) -> f32 {
        self.weights.get(expression).copied().unwrap_or(0.0)
    }

    pub fn set_weight(&mut self, expression: usize, weight: f32) {
        if let Some(slot) = self.weights.get_mut(expression) {
            *slot = weight.clamp(0.0, 1.0);
        }
    }

    pub fn clear_weights(&mut self) {
        self.weights.fill(0.0);
    }

    #[must_use]
    pub fn look_at(&self) -> Option<Vec3> {
        self.look_at
    }

    pub fn set_look_at(&mut self, target: Option<Vec3>) {
        self.look_at = target;
    }

    #[must_use]
    pub fn gaze(&self) -> Option<Gaze> {
        self.gaze
    }

    pub fn set_gaze(&mut self, gaze: Option<Gaze>) {
        self.gaze = gaze;
    }
}
impl Rig for Avatar {
    fn evaluate(&mut self, model: &Model, transform: Mat4, state: PoseState<'_>, phase: Phase) {
        let Some(vrm) = model.extensions.get::<Vrm>() else {
            return;
        };
        let PoseState {
            locals,
            world,
            weights,
            ..
        } = state;
        match phase {
            Phase::BeforePhysics => walk(model, self, vrm, locals, world),
            Phase::AfterPhysics => express(model, vrm, self, transform, locals, world, weights),
        }
    }

    fn duplicate(&self) -> Box<dyn Rig> {
        Box::new(self.clone())
    }

    fn tints(&self) -> Option<&Tints> {
        Some(&self.tints)
    }
}

fn express(
    model: &Model,
    vrm: &Vrm,
    avatar: &mut Avatar,
    transform: Mat4,
    locals: &mut [Transform],
    world: &mut [Mat4],
    weights: &mut [Vec<f32>],
) {
    let gaze = gaze(model, avatar, vrm, transform, locals, world);
    if gaze.is_some() && vrm.look_at.kind == LookAtKind::Bone {
        walk(model, avatar, vrm, locals, world);
    }

    resolve(
        vrm,
        avatar,
        gaze.filter(|_| vrm.look_at.kind == LookAtKind::Expression),
    );
    morphs(vrm, avatar, weights);
    tint(model, vrm, avatar);
}

fn order(model: &Model, vrm: &Vrm, constrained: &[Option<usize>]) -> Vec<NodeId> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Open,
        Walking,
        Done,
    }

    let mut marks = vec![Mark::Open; model.nodes.len()];
    let mut order = Vec::with_capacity(model.nodes.len());
    let mut stack: Vec<(NodeId, bool)> = (0..model.nodes.len())
        .map(|index| (NodeId(index as u32), false))
        .rev()
        .collect();

    while let Some((node, revisited)) = stack.pop() {
        match (marks[node.index()], revisited) {
            (Mark::Done, _) => continue,
            (Mark::Walking, false) => continue,
            (_, true) => {
                marks[node.index()] = Mark::Done;
                order.push(node);
                continue;
            }
            (Mark::Open, false) => marks[node.index()] = Mark::Walking,
        }

        stack.push((node, true));
        let source =
            constrained[node.index()].map(|index| vrm.constraints[index].constraint.source());
        let before = model.node(node).parent.into_iter().chain(source);
        for dependency in before {
            if marks[dependency.index()] == Mark::Open {
                stack.push((dependency, false));
            }
        }
    }
    order
}
fn walk(model: &Model, avatar: &Avatar, vrm: &Vrm, locals: &mut [Transform], world: &mut [Mat4]) {
    for (node, constrained) in &avatar.order {
        if let Some(constraint) = constrained.map(|index| &vrm.constraints[index].constraint) {
            let index = node.index();
            let above = model.parent_world(*node, world);
            let source = constraint.source();
            let state = ConstraintState {
                source: locals[source.index()].rotation,
                source_rest: model.node(source).transform.rotation,
                destination_rest: model.node(*node).transform.rotation,
                parent_world: Transform::from_matrix(above).rotation,
                source_world_position: world[source.index()].w_axis.truncate(),
                destination_world_position: (above * locals[index].to_matrix()).w_axis.truncate(),
            };
            locals[index].rotation = constraint.resolve(&state);
        }
        model.place(*node, locals, world);
    }
}

fn gaze(
    model: &Model,
    avatar: &Avatar,
    vrm: &Vrm,
    transform: Mat4,
    locals: &mut [Transform],
    world: &[Mat4],
) -> Option<(f32, f32)> {
    let head = vrm.bone(HumanBone::Head)?;
    let head_world = *world.get(head.index())?;

    let (yaw, pitch) = match (avatar.gaze, avatar.look_at) {
        (Some(gaze), _) => (gaze.yaw.to_degrees(), gaze.pitch.to_degrees()),
        (None, Some(target)) => {
            let facing = Transform::from_matrix(head_world).rotation * avatar.head_rest.inverse();
            let origin = head_world.transform_point3(vrm.look_at.offset_from_head);
            let local = facing
                .inverse()
                .mul_vec3(transform.inverse().transform_point3(target) - origin);
            let flat = (local.x * local.x + local.z * local.z).sqrt();
            (
                local.x.atan2(local.z).to_degrees(),
                (-local.y).atan2(flat).to_degrees(),
            )
        }
        (None, None) => return None,
    };

    if vrm.look_at.kind == LookAtKind::Bone {
        let vertical = match pitch >= 0.0 {
            true => vrm.look_at.vertical_down.apply(pitch),
            false => -vrm.look_at.vertical_up.apply(pitch),
        };
        let sideways = [
            (vrm.look_at.horizontal_inner, vrm.look_at.horizontal_outer),
            (vrm.look_at.horizontal_outer, vrm.look_at.horizontal_inner),
        ];
        for (eye, (inner, outer)) in avatar.eyes.iter().zip(sideways) {
            let Some(eye) = eye else {
                continue;
            };
            let horizontal = match yaw >= 0.0 {
                true => outer.apply(yaw),
                false => -inner.apply(yaw),
            };
            let turn = Quat::from_euler(
                EulerRot::YXZ,
                horizontal.to_radians(),
                vertical.to_radians(),
                0.0,
            );
            let rest = model.node(eye.node).transform.rotation;
            if let Some(local) = locals.get_mut(eye.node.index()) {
                local.rotation = eye.parent_rest.inverse() * turn * eye.parent_rest * rest;
            }
        }
    }
    Some((yaw, pitch))
}

fn resolve(vrm: &Vrm, avatar: &mut Avatar, gaze: Option<(f32, f32)>) {
    for (index, expression) in vrm.expressions.iter().enumerate() {
        let mut weight = avatar.weights.get(index).copied().unwrap_or(0.0);
        if let (Some((yaw, pitch)), Some(preset)) = (gaze, expression.name.preset()) {
            weight = weight.max(look_at_weight(vrm, preset, yaw, pitch));
        }
        avatar.resolved[index] = expression.weigh(weight);
    }

    for kind in Procedural::ALL.iter().copied() {
        let mut blocked = false;
        let mut blended = 0.0;
        for (index, expression) in vrm.expressions.iter().enumerate() {
            if expression.is_procedural(kind) {
                continue;
            }
            match expression.override_of(kind) {
                ExpressionOverride::Block => blocked |= avatar.resolved[index] > 0.0,
                ExpressionOverride::Blend => blended += avatar.resolved[index],
                ExpressionOverride::None => {}
            }
        }

        let factor = match blocked {
            true => 0.0,
            false => (1.0 - blended).clamp(0.0, 1.0),
        };
        if factor >= 1.0 {
            continue;
        }
        for (index, expression) in vrm.expressions.iter().enumerate() {
            if !expression.is_procedural(kind) {
                continue;
            }
            avatar.resolved[index] = match expression.is_binary {
                true => 0.0,
                false => avatar.resolved[index] * factor,
            };
        }
    }
}

fn look_at_weight(vrm: &Vrm, preset: ExpressionPreset, yaw: f32, pitch: f32) -> f32 {
    match preset {
        ExpressionPreset::LookLeft if yaw > 0.0 => vrm.look_at.horizontal_outer.apply(yaw),
        ExpressionPreset::LookRight if yaw < 0.0 => vrm.look_at.horizontal_outer.apply(yaw),
        ExpressionPreset::LookDown if pitch > 0.0 => vrm.look_at.vertical_down.apply(pitch),
        ExpressionPreset::LookUp if pitch < 0.0 => vrm.look_at.vertical_up.apply(pitch),
        _ => 0.0,
    }
}

fn morphs(vrm: &Vrm, avatar: &Avatar, weights: &mut [Vec<f32>]) {
    for (index, expression) in vrm.expressions.iter().enumerate() {
        let weight = avatar.resolved[index];
        if weight <= 0.0 {
            continue;
        }
        for bind in &expression.morph_binds {
            let weight_slot = weights
                .get_mut(bind.node.index())
                .and_then(|node_weights| node_weights.get_mut(bind.index as usize));
            if let Some(slot) = weight_slot {
                *slot += bind.weight * weight;
            }
        }
    }
}

fn tint(model: &Model, vrm: &Vrm, avatar: &mut Avatar) {
    if avatar.tints.is_empty() {
        return;
    }
    let tinting = vrm
        .expressions
        .iter()
        .zip(&avatar.resolved)
        .any(|(expression, weight)| {
            *weight > 0.0
                && !(expression.material_color_binds.is_empty()
                    && expression.texture_transform_binds.is_empty())
        });
    let was = std::mem::replace(&mut avatar.tinted, tinting);
    if !tinting && !was {
        return;
    }
    avatar.tints.restore(model);

    for (index, expression) in vrm.expressions.iter().enumerate() {
        let weight = avatar.resolved[index];
        if weight <= 0.0 {
            continue;
        }
        for bind in &expression.material_color_binds {
            let base = model.material(bind.material);
            if let Some(material) = avatar.tints.get_mut(bind.material) {
                apply_color(material, base, bind.color, bind.value, weight);
            }
        }
        for bind in &expression.texture_transform_binds {
            let base = model.material(bind.material).uv_transform;
            if let Some(material) = avatar.tints.get_mut(bind.material) {
                material.uv_transform.offset += (bind.offset - base.offset) * weight;
                material.uv_transform.scale += (bind.scale - base.scale) * weight;
            }
        }
    }
}

fn bound_materials(model: &Model, vrm: &Vrm) -> Tints {
    let bound = vrm.expressions.iter().flat_map(|expression| {
        let colors = expression
            .material_color_binds
            .iter()
            .map(|bind| bind.material);
        let placements = expression
            .texture_transform_binds
            .iter()
            .map(|bind| bind.material);
        colors.chain(placements)
    });
    Tints::for_materials(model, bound)
}

fn apply_color(
    material: &mut Material,
    base: &Material,
    color: MaterialColor,
    value: rikka_model_core::glam::Vec4,
    weight: f32,
) {
    if color == MaterialColor::Base {
        material.base_color += (value - base.base_color) * weight;
        return;
    }
    if color == MaterialColor::Emissive {
        material.emissive += (value.truncate() - base.emissive) * weight;
        return;
    }

    let (Shading::Toon(toon), Shading::Toon(rest)) = (&mut material.shading, base.shading) else {
        return;
    };
    let target = value.truncate();
    match color {
        MaterialColor::Shade => toon.shade_color += (target - rest.shade_color) * weight,
        MaterialColor::Matcap => toon.matcap_color += (target - rest.matcap_color) * weight,
        MaterialColor::Rim => toon.rim_color += (target - rest.rim_color) * weight,
        MaterialColor::Outline => {
            toon.outline.color += (target - rest.outline.color) * weight;
        }
        MaterialColor::Base | MaterialColor::Emissive => {}
    }
}
