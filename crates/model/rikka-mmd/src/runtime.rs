use rikka_model_core::glam::{EulerRot, Mat3, Mat4, Quat, Vec3};

use rikka_model_core::{MaterialId, Model, NodeId, Phase, PoseState, Rig, Tints, Transform};

use super::pmx::surface::Surface;
use super::pmx::{Pmx, PmxBoneFlags, PmxIk, PmxOffsets};

#[expect(
    clippy::approx_constant,
    reason = "MMD 9.32 bounds IK steps by 3.14, not π"
)]
const HALF_TURN: f32 = 3.14;

const STEP_SCALE: f32 = 0.25;
const STEP_BOUND: f32 = 2.0;
const GIMBAL_BOUND: f32 = 1.535_889;

const REACHED: f32 = 1e-7;
const SHORTEST: f32 = 1.192_092_9e-7;

const MAX_PASSES: f32 = 360.0;

#[derive(Clone, Debug)]
pub struct Puppet {
    order: Vec<usize>,
    reaching: Vec<usize>,
    after: usize,
    reaching_after: usize,
    ik: Vec<Quat>,
    resting: Vec<bool>,
    weights: Vec<f32>,
    moved: Vec<(usize, Transform)>,
    chain: Vec<NodeId>,
    tints: Tints,
    tinted: bool,
    rest: Vec<Vec3>,
}

impl Puppet {
    #[must_use]
    pub fn new(model: &Model) -> Option<Self> {
        let pmx = model.extensions.get::<Pmx>()?;
        if pmx
            .bones
            .iter()
            .any(|bone| bone.node.index() >= model.nodes.len())
        {
            return None;
        }
        let late = |bone: usize| pmx.bones[bone].flags.has(PmxBoneFlags::PHYSICS_AFTER);
        let mut order: Vec<usize> = (0..pmx.bones.len()).collect();
        order.sort_by_key(|bone| (late(*bone), pmx.bones[*bone].layer, *bone));
        let reaching: Vec<usize> = order
            .iter()
            .copied()
            .filter(|bone| pmx.bones[*bone].ik.is_some())
            .collect();
        Some(Self {
            after: order.iter().take_while(|bone| !late(**bone)).count(),
            reaching_after: reaching.iter().take_while(|bone| !late(**bone)).count(),
            order,
            reaching,
            ik: vec![Quat::IDENTITY; pmx.bones.len()],
            resting: vec![false; pmx.bones.len()],
            weights: vec![0.0; pmx.morphs.len()],
            moved: Vec::new(),
            chain: Vec::new(),
            tints: bound_materials(model, pmx),
            tinted: false,
            rest: model
                .nodes
                .iter()
                .map(|node| node.transform.translation)
                .collect(),
        })
    }

    #[must_use]
    pub fn is_reaching(&self, bone: usize) -> bool {
        self.resting.get(bone).is_some_and(|resting| !resting)
    }

    pub fn set_reaching(&mut self, bone: usize, reaching: bool) {
        if let Some(resting) = self.resting.get_mut(bone) {
            *resting = !reaching;
        }
    }

    pub fn reach_everywhere(&mut self) {
        self.resting.fill(false);
    }
}

impl Rig for Puppet {
    fn evaluate(&mut self, model: &Model, _transform: Mat4, state: PoseState<'_>, phase: Phase) {
        let Some(pmx) = model.extensions.get::<Pmx>() else {
            return;
        };
        evaluate(model, pmx, self, state, phase);
    }

    fn duplicate(&self) -> Box<dyn Rig> {
        Box::new(self.clone())
    }

    fn tints(&self) -> Option<&Tints> {
        Some(&self.tints)
    }
}

fn bound_materials(model: &Model, pmx: &Pmx) -> Tints {
    let every = 0..model.materials.len() as u32;
    let bound = pmx
        .morphs
        .iter()
        .filter_map(|morph| match &morph.offsets {
            PmxOffsets::Material(offsets) => Some(offsets),
            _ => None,
        })
        .flatten()
        .flat_map(|offset| match offset.material {
            Some(material) => material as u32..material as u32 + 1,
            None => every.clone(),
        })
        .map(MaterialId);
    Tints::for_materials(model, bound)
}

fn evaluate(model: &Model, pmx: &Pmx, rig: &mut Puppet, state: PoseState<'_>, phase: Phase) {
    let PoseState {
        order,
        locals,
        world,
        weights,
    } = state;

    let (bones, iks) = match phase {
        Phase::BeforePhysics => {
            resolve_weights(pmx, rig, weights);
            bone_offsets(pmx, rig);
            apply_material_morphs(model, pmx, rig);
            for (bone, offset) in &rig.moved {
                if let Some(local) = locals.get_mut(pmx.bones[*bone].node.index()) {
                    local.translation += offset.translation;
                    local.rotation = (local.rotation * offset.rotation).normalize();
                }
            }
            model.place_all(order, locals, world);
            (0..rig.after, 0..rig.reaching_after)
        }
        Phase::AfterPhysics if rig.after == rig.order.len() => return,
        Phase::AfterPhysics => (
            rig.after..rig.order.len(),
            rig.reaching_after..rig.reaching.len(),
        ),
    };

    let mut reaching = iks.start;
    for index in bones.clone() {
        let bone = rig.order[index];
        let layer = pmx.bones[bone].layer;
        append(pmx, bone, locals, &rig.rest);
        model.place(pmx.bones[bone].node, locals, world);

        let last = index + 1 == bones.end || pmx.bones[rig.order[index + 1]].layer != layer;
        while last && reaching < iks.end {
            let ik_bone = rig.reaching[reaching];
            if pmx.bones[ik_bone].layer > layer {
                break;
            }
            if !rig.resting[ik_bone] {
                reach(model, pmx, rig, ik_bone, locals, world);
            }
            reaching += 1;
        }
    }
    model.place_all(order, locals, world);
}

fn resolve_weights(pmx: &Pmx, rig: &mut Puppet, weights: &mut [Vec<f32>]) {
    let Some(root_weights) = weights.get_mut(pmx.root.index()) else {
        return;
    };
    root_weights.resize(pmx.morphs.len(), 0.0);
    rig.weights.clear();
    rig.weights.extend_from_slice(root_weights);

    for round in 0..8 {
        let mut moved = false;
        for (index, morph) in pmx.morphs.iter().enumerate() {
            let PmxOffsets::Group(offsets) = &morph.offsets else {
                continue;
            };
            let weight = rig.weights[index];
            if weight == 0.0 {
                continue;
            }
            for offset in offsets {
                if offset.morph == index || offset.morph >= rig.weights.len() {
                    continue;
                }
                rig.weights[offset.morph] += weight * offset.ratio;
                moved = true;
            }
            rig.weights[index] = 0.0;
        }
        if !moved || round == 7 {
            break;
        }
    }

    for (slot, weight) in root_weights.iter_mut().zip(&rig.weights) {
        *slot = *weight;
    }
}

fn bone_offsets(pmx: &Pmx, rig: &mut Puppet) {
    rig.moved.clear();
    for (index, morph) in pmx.morphs.iter().enumerate() {
        let PmxOffsets::Bone(offsets) = &morph.offsets else {
            continue;
        };
        let weight = rig.weights.get(index).copied().unwrap_or(0.0);
        if weight == 0.0 {
            continue;
        }
        for offset in offsets {
            if offset.bone >= pmx.bones.len() {
                continue;
            }
            let turn = Quat::IDENTITY.slerp(offset.rotation, weight);
            match rig.moved.iter_mut().find(|(bone, _)| *bone == offset.bone) {
                Some((_, offset_transform)) => {
                    offset_transform.translation += offset.translation * weight;
                    offset_transform.rotation = (offset_transform.rotation * turn).normalize();
                }
                None => rig.moved.push((
                    offset.bone,
                    Transform {
                        translation: offset.translation * weight,
                        rotation: turn,
                        scale: Vec3::ONE,
                    },
                )),
            }
        }
    }
}

fn apply_material_morphs(model: &Model, pmx: &Pmx, rig: &mut Puppet) {
    let tinting = pmx.morphs.iter().enumerate().any(|(index, morph)| {
        matches!(morph.offsets, PmxOffsets::Material(_))
            && rig.weights.get(index).copied().unwrap_or(0.0) != 0.0
    });
    let was = std::mem::replace(&mut rig.tinted, tinting);
    if !tinting && !was {
        return;
    }
    for (id, material) in rig.tints.iter_mut() {
        material.clone_from(model.material(id));
        let Some(pmx_material) = pmx.materials.get(id.index()) else {
            continue;
        };
        let mut surface = Surface::from_material(pmx_material);
        for (index, morph) in pmx.morphs.iter().enumerate() {
            let PmxOffsets::Material(offsets) = &morph.offsets else {
                continue;
            };
            let weight = rig.weights.get(index).copied().unwrap_or(0.0);
            if weight == 0.0 {
                continue;
            }
            for offset in offsets
                .iter()
                .filter(|offset| offset.material.is_none_or(|named| named == id.index()))
            {
                surface.blend(offset, weight);
            }
        }
        surface.paint(material);
    }
}

fn append(pmx: &Pmx, bone: usize, locals: &mut [Transform], rest: &[Vec3]) {
    let pmx_bone = &pmx.bones[bone];
    let Some(append) = pmx_bone
        .append
        .filter(|append| append.parent < pmx.bones.len())
    else {
        return;
    };
    let node = pmx_bone.node.index();
    let above = pmx.bones[append.parent].node.index();

    if pmx_bone.flags.has(PmxBoneFlags::APPEND_ROTATE) {
        let taken = shared(locals[above].rotation, append.ratio);
        locals[node].rotation = (locals[node].rotation * taken).normalize();
    }
    if pmx_bone.flags.has(PmxBoneFlags::APPEND_TRANSLATE) {
        let moved = locals[above].translation - rest[above];
        locals[node].translation += moved * append.ratio;
    }
}

fn shared(rotation: Quat, ratio: f32) -> Quat {
    let unit = rotation.normalize();
    let mut angle = unit.w.clamp(-1.0, 1.0).acos();
    if unit.w < 0.0 {
        angle -= HALF_TURN;
    }
    let axis = Vec3::new(unit.x, unit.y, unit.z);
    let length = axis.length();
    if angle == 0.0 || length < SHORTEST {
        return Quat::IDENTITY;
    }
    let angle = angle * ratio;
    let scale = angle.sin() / length;
    Quat::from_xyzw(axis.x * scale, axis.y * scale, axis.z * scale, angle.cos())
}

fn reach(
    model: &Model,
    pmx: &Pmx,
    rig: &mut Puppet,
    bone: usize,
    locals: &mut [Transform],
    world: &mut [Mat4],
) {
    let pmx_bone = &pmx.bones[bone];
    let Some(ik) = &pmx_bone.ik else {
        return;
    };
    if ik.target >= pmx.bones.len() || ik.links.is_empty() {
        return;
    }
    let target = pmx.bones[ik.target].node;
    let goal = world[pmx_bone.node.index()].w_axis.truncate();
    let step = ik.limit_angle * STEP_SCALE;
    let passes = passes(ik, step);

    let Puppet {
        ik: turns, chain, ..
    } = rig;
    gather_chain(model, pmx, ik, target, chain);
    for link in &ik.links {
        if let Some(turn) = turns.get_mut(link.bone) {
            *turn = Quat::IDENTITY;
        }
    }

    for pass in 0..passes {
        let hard = pass * 2 >= passes;
        let mut reached = false;
        for (depth, link) in ik.links.iter().enumerate() {
            if link.bone >= pmx.bones.len() {
                continue;
            }
            let node = pmx.bones[link.bone].node;
            let effector = world[target.index()].w_axis.truncate();
            let standing = world[node.index()].w_axis.truncate();
            let to_effector = (standing - effector).normalize_or_zero();
            let to_goal = (standing - goal).normalize_or_zero();
            if (to_effector - to_goal).length_squared() < REACHED {
                reached = true;
                break;
            }

            let parent = Mat3::from_mat4(model.parent_world(node, world));
            let axis = to_effector.cross(to_goal);
            let mut turned = (parent.transpose() * axis).normalize_or_zero();
            let mut factor = 1.0;
            if let Some(fixed) = pmx.bones[link.bone].fixed_axis {
                factor = turned.dot(fixed).signum();
                turned = fixed;
            } else if let Some(limits) = link.limits.filter(|_| !hard) {
                turned = restricted(turned, limits.lower, limits.upper).unwrap_or(turned);
            }
            if turned.length_squared() < SHORTEST {
                continue;
            }

            let angle = to_effector.dot(to_goal).clamp(-1.0, 1.0).acos() * 0.5 * factor;
            let bound = ((depth + 1) as f32 * step * STEP_BOUND).abs();
            let angle = angle.clamp(-bound, bound);
            let (sine, cosine) = angle.sin_cos();
            let turn = Quat::from_xyzw(turned.x * sine, turned.y * sine, turned.z * sine, cosine);

            let mut rotation = turn * turns[link.bone];
            if pass == 0 {
                rotation *= locals[node.index()].rotation;
            }
            if let Some(limits) = link.limits {
                rotation = limited(rotation, limits.lower, limits.upper, hard);
            }
            turns[link.bone] = rotation.normalize();
            locals[node.index()].rotation = turns[link.bone];

            for node in chain.iter() {
                model.place(*node, locals, world);
            }
        }
        if reached {
            break;
        }
    }
}

fn gather_chain(model: &Model, pmx: &Pmx, ik: &PmxIk, target: NodeId, chain: &mut Vec<NodeId>) {
    chain.clear();
    let gather = |node: NodeId, chain: &mut Vec<NodeId>| {
        let first = chain.len();
        let mut cursor = Some(node);
        while let Some(node) = cursor.filter(|node| !chain.contains(node)) {
            chain.push(node);
            cursor = model.node(node).parent;
        }
        chain[first..].reverse();
    };
    gather(target, chain);
    for link in &ik.links {
        if let Some(bone) = pmx.bones.get(link.bone) {
            gather(bone.node, chain);
        }
    }
}

fn passes(ik: &PmxIk, step: f32) -> usize {
    let mut passes = ik.loops as f32;
    if step != 0.0 {
        passes = passes.max((std::f32::consts::PI / step).min(MAX_PASSES));
    }
    passes.max(1.0) as usize
}

fn restricted(axis: Vec3, lower: Vec3, upper: Vec3) -> Option<Vec3> {
    let locked = |component: usize| lower[component] == 0.0 && upper[component] == 0.0;
    let free = match (locked(0), locked(1), locked(2)) {
        (false, true, true) => Vec3::X,
        (true, false, true) => Vec3::Y,
        (true, true, false) => Vec3::Z,
        _ => return None,
    };
    Some(free * axis.dot(free).signum())
}

fn limited(rotation: Quat, lower: Vec3, upper: Vec3, hard: bool) -> Quat {
    let narrow = |axis: usize| {
        lower[axis] > -std::f32::consts::FRAC_PI_2 && upper[axis] < std::f32::consts::FRAC_PI_2
    };
    let (order, angles, pivot) = if narrow(0) {
        let (y, x, z) = rotation.to_euler(EulerRot::YXZ);
        (EulerRot::YXZ, Vec3::new(x, y, z), 0)
    } else if narrow(1) {
        let (z, y, x) = rotation.to_euler(EulerRot::ZYX);
        (EulerRot::ZYX, Vec3::new(x, y, z), 1)
    } else {
        let (x, z, y) = rotation.to_euler(EulerRot::XZY);
        (EulerRot::XZY, Vec3::new(x, y, z), 2)
    };

    let mut clamped = angles;
    clamped[pivot] = clamped[pivot].clamp(-GIMBAL_BOUND, GIMBAL_BOUND);
    for axis in 0..3 {
        clamped[axis] = inside(clamped[axis], lower[axis], upper[axis], hard);
    }
    match order {
        EulerRot::YXZ => Quat::from_euler(order, clamped.y, clamped.x, clamped.z),
        EulerRot::ZYX => Quat::from_euler(order, clamped.z, clamped.y, clamped.x),
        _ => Quat::from_euler(EulerRot::XZY, clamped.x, clamped.z, clamped.y),
    }
    .normalize()
}

fn inside(angle: f32, lower: f32, upper: f32, hard: bool) -> f32 {
    let mut folded = angle;
    if folded < lower {
        let reflected = lower * 2.0 - folded;
        folded = match reflected > upper || hard {
            true => lower,
            false => reflected,
        };
    }
    if upper < folded {
        let reflected = upper * 2.0 - folded;
        folded = match reflected < lower || hard {
            true => upper,
            false => reflected,
        };
    }
    folded
}
