use rikka_model_core::glam::{Mat4, Quat, Vec3};
use rikka_model_core::{HumanBone, Model, NodeId, Transform};

use crate::hierarchy::{HUMAN_BONES, aims};
use crate::mapping::{Append, Mapping};
use crate::pose::HumanPose;
use crate::skeleton::{Skeleton, position_of, rotation_of};
use crate::turn::{canonical, split};

#[expect(clippy::approx_constant)]
const HALF_TURN: f32 = 3.14;

const SHORTEST: f32 = 1.192_092_9e-7;

#[derive(Clone, Copy, Debug)]
struct Twist {
    bone: HumanBone,
    node: NodeId,
    axis: Vec3,
}

#[derive(Clone, Debug)]
pub(crate) struct Binding {
    driven: Vec<NodeId>,
    parents: Vec<Option<NodeId>>,
    rests: Vec<Transform>,
    appends: Vec<Option<Append>>,
    humans: Vec<(HumanBone, NodeId)>,
    twists: Vec<Twist>,
    carrier: NodeId,
    hips: NodeId,
    base: Mat4,
    lift: Mat4,
    girth: Vec3,
}

impl Binding {
    pub(crate) fn new(model: &Model, mapping: &Mapping, rest: &[Mat4]) -> Self {
        let carrier = mapping.carrier();
        let hips = mapping.node(HumanBone::Hips).unwrap_or(carrier);
        let humans: Vec<(HumanBone, NodeId)> = HumanBone::ALL
            .iter()
            .filter_map(|bone| Some((*bone, mapping.node(*bone)?)))
            .collect();
        let twists: Vec<Twist> = mapping
            .twists()
            .filter_map(|(bone, node)| {
                let twisted = mapping.node(bone)?;
                let below = aims(bone).into_iter().find_map(|aim| mapping.node(aim))?;
                let along = position_of(rest[below.index()]) - position_of(rest[twisted.index()]);
                let axis =
                    (rotation_of(rest[twisted.index()]).inverse() * along).try_normalize()?;
                Some(Twist { bone, node, axis })
            })
            .collect();
        let mut driven = vec![carrier];
        driven.extend(mapping.intermediates().iter().copied());
        driven.extend(
            humans
                .iter()
                .map(|(_, node)| *node)
                .filter(|node| *node != carrier),
        );
        driven.extend(twists.iter().map(|twist| twist.node));
        let mut binding = Self {
            driven,
            parents: model.nodes.iter().map(|node| node.parent).collect(),
            rests: model.nodes.iter().map(|node| node.transform).collect(),
            appends: (0..model.nodes.len())
                .map(|index| mapping.append(NodeId(index as u32)))
                .collect(),
            humans,
            twists,
            carrier,
            hips,
            base: Mat4::IDENTITY,
            lift: Mat4::IDENTITY,
            girth: rest[hips.index()].to_scale_rotation_translation().0,
        };
        let rests = binding.rests.clone();
        binding.base = model
            .node(carrier)
            .parent
            .map_or(Mat4::IDENTITY, |parent| binding.world(parent, &rests));
        binding.lift = binding.world(carrier, &rests).inverse() * binding.world(hips, &rests);
        binding
    }

    pub(crate) fn driven_nodes(&self) -> &[NodeId] {
        &self.driven
    }

    pub(crate) fn locals(&self, skeleton: &Skeleton, pose: &HumanPose) -> Vec<Transform> {
        let worlds = skeleton.fk(pose);
        let mut locals = self.rests.clone();
        let hips = worlds.joint(HumanBone::Hips);
        let target = Mat4::from_scale_rotation_translation(
            self.girth,
            hips.rotation * skeleton.frame(HumanBone::Hips).tpose,
            hips.position,
        );
        let carried = Transform::from_matrix(self.base.inverse() * target * self.lift.inverse());
        locals[self.carrier.index()] = Transform {
            rotation: canonical(carried.rotation),
            ..carried
        };
        for (bone, node) in &self.humans {
            if *node == self.carrier || *node == self.hips {
                continue;
            }
            let Some(joint) = worlds.get(*bone) else {
                continue;
            };
            let desired = joint.rotation * skeleton.frame(*bone).tpose;
            let above = self.parents[node.index()].map_or(Quat::IDENTITY, |parent| {
                rotation_of(self.world(parent, &locals))
            });
            let local = above.inverse() * desired;
            match self.twists.iter().find(|twist| twist.bone == *bone) {
                Some(twist) => {
                    let (swing, angle) = split(local, twist.axis);
                    locals[node.index()].rotation = swing;
                    let rested = self.rests[twist.node.index()].rotation;
                    locals[twist.node.index()].rotation =
                        canonical(Quat::from_axis_angle(twist.axis, angle) * rested);
                }
                None => locals[node.index()].rotation = canonical(local),
            }
        }
        self.driven
            .iter()
            .map(|node| locals[node.index()])
            .collect()
    }

    pub(crate) fn read(&self, skeleton: &Skeleton, world: impl Fn(NodeId) -> Mat4) -> HumanPose {
        let mut pose = HumanPose::default();
        let mut normal = [Quat::IDENTITY; HUMAN_BONES];
        for (bone, node) in &self.humans {
            let world_rotation = match self.twists.iter().find(|twist| twist.bone == *bone) {
                Some(twist) => {
                    rotation_of(world(twist.node))
                        * self.rests[twist.node.index()].rotation.inverse()
                }
                None => rotation_of(world(*node)),
            };
            let frame = skeleton.frame(*bone);
            let normalized = (world_rotation * frame.tpose.inverse()).normalize();
            normal[bone.index()] = normalized;
            match frame.parent {
                Some(parent) if *bone != HumanBone::Hips => {
                    let local = normal[parent.index()].inverse() * normalized;
                    skeleton.set_local_rotation(*bone, local, &mut pose);
                }
                _ => {
                    pose.root_rot = canonical(normalized);
                    pose.root_pos = position_of(world(*node)) / skeleton.scale();
                }
            }
        }
        pose
    }

    fn effective(&self, node: NodeId, locals: &[Transform], depth: usize) -> Transform {
        let mut transform = locals[node.index()];
        let Some(append) = self.appends[node.index()] else {
            return transform;
        };
        if depth > self.rests.len() {
            return transform;
        }
        let source = self.effective(append.source, locals, depth + 1);
        if append.rotates {
            transform.rotation =
                (transform.rotation * shared(source.rotation, append.ratio)).normalize();
        }
        if append.moves {
            let moved = source.translation - self.rests[append.source.index()].translation;
            transform.translation += moved * append.ratio;
        }
        transform
    }

    fn world(&self, node: NodeId, locals: &[Transform]) -> Mat4 {
        let mut chain = vec![node];
        while let Some(parent) = chain.last().and_then(|last| self.parents[last.index()]) {
            if chain.len() > self.parents.len() {
                break;
            }
            chain.push(parent);
        }
        chain.iter().rev().fold(Mat4::IDENTITY, |above, node| {
            above * self.effective(*node, locals, 0).to_matrix()
        })
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
