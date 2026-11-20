use rikka_mmd::{Pmx, PmxTail};
use rikka_model_core::glam::{Mat4, Quat, Vec3};
use rikka_model_core::{HumanBone, Model, NodeId};

use crate::hierarchy::{HUMAN_BONES, aims, heading, parents};
use crate::mapping::Mapping;
use crate::muscle::{Dof, muscle_of, muscles};
use crate::pose::HumanPose;
use crate::turn::split;
use crate::worlds::{Joint, Worlds};

const SHORT: f32 = 1e-6;
const LONGEST_SWING: f32 = std::f32::consts::PI * 0.97;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub(crate) present: bool,
    pub(crate) parent: Option<HumanBone>,
    pub(crate) offset: Vec3,
    pub(crate) tpose: Quat,
    pub(crate) rest: Quat,
    pub(crate) direction: Vec3,
    pub(crate) reach: Option<Vec3>,
    pub(crate) axes: [Vec3; 3],
    pub(crate) muscles: [Option<usize>; 3],
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            present: false,
            parent: None,
            offset: Vec3::ZERO,
            tpose: Quat::IDENTITY,
            rest: Quat::IDENTITY,
            direction: Vec3::Y,
            reach: None,
            axes: [Vec3::X, Vec3::Z, Vec3::Y],
            muscles: [None; 3],
        }
    }
}

const DOFS: [Dof; 3] = [Dof::Bend, Dof::Side, Dof::Twist];

#[derive(Clone, Debug)]
pub(crate) struct Skeleton {
    frames: [Frame; HUMAN_BONES],
    scale: f32,
}

pub(crate) fn rotation_of(matrix: Mat4) -> Quat {
    matrix.to_scale_rotation_translation().1.normalize()
}

pub(crate) fn position_of(matrix: Mat4) -> Vec3 {
    matrix.w_axis.truncate()
}

fn target(bone: HumanBone, current: Vec3) -> Vec3 {
    match bone {
        HumanBone::LeftFoot | HumanBone::RightFoot => {
            let ahead = Vec3::new(current.x, 0.0, current.z).length();
            Vec3::new(0.0, current.y, ahead).normalize_or(heading(bone))
        }
        _ => heading(bone),
    }
}

pub(crate) fn tip(model: &Model, node: NodeId, rest: &[Mat4]) -> Option<Vec3> {
    let place = |node: NodeId| rest.get(node.index()).copied().map(position_of);
    let tail = match model.extensions.get::<Pmx>() {
        Some(pmx) => {
            let bone = pmx.bones.iter().find(|bone| bone.node == node)?;
            match bone.tail {
                PmxTail::Offset(offset) => offset,
                PmxTail::Bone(Some(index)) => place(pmx.bones.get(index)?.node)? - place(node)?,
                PmxTail::Bone(None) => return None,
            }
        }
        None => match model.node(node).children.as_slice() {
            [child] => place(*child)? - place(node)?,
            _ => return None,
        },
    };
    Some(tail).filter(|tail| tail.length() > SHORT)
}

impl Skeleton {
    pub(crate) fn new(model: &Model, mapping: &Mapping, rest: &[Mat4]) -> Self {
        let mut frames = [Frame::default(); HUMAN_BONES];
        let place = |node: NodeId| position_of(rest[node.index()]);
        let turn = |node: NodeId| rotation_of(rest[node.index()]);
        for bone in HumanBone::ALL.iter().copied() {
            let parent = parents(bone)
                .into_iter()
                .find(|parent| mapping.node(*parent).is_some());
            let mut frame = Frame {
                parent,
                muscles: DOFS.map(|dof| muscle_of(bone, dof)),
                ..Frame::default()
            };
            let Some(node) = mapping.node(bone) else {
                frame.direction = heading(bone);
                frame.axes = axes(bone, frame.direction, &frame.muscles);
                frames[bone.index()] = frame;
                continue;
            };
            frame.present = true;
            frame.rest = turn(node);
            let aim = aims(bone)
                .into_iter()
                .find_map(|aim| mapping.node(aim))
                .map(|aim| place(aim) - place(node))
                .filter(|offset| offset.length() > SHORT);
            let along = match bone {
                HumanBone::Hips | HumanBone::LeftEye | HumanBone::RightEye => None,
                _ => aim.or_else(|| tip(model, node, rest)),
            };
            let above = parent.and_then(|parent| {
                let node = mapping.node(parent)?;
                Some((frames[parent.index()].tpose, turn(node), place(node)))
            });
            let unturned = match (bone, above) {
                (HumanBone::Hips, _) | (_, None) => frame.rest,
                (_, Some((tpose, turned, _))) => tpose * turned.inverse() * frame.rest,
            };
            let swings = frame.muscles[0].is_some() && frame.muscles[1].is_some();
            frame.tpose = match (aim, swings) {
                (Some(aim), true) => {
                    let current = (unturned * frame.rest.inverse() * aim).normalize();
                    (Quat::from_rotation_arc(current, target(bone, current)) * unturned).normalize()
                }
                _ => unturned,
            };
            let normal = |direction: Vec3| frame.tpose * frame.rest.inverse() * direction;
            frame.reach = along.map(normal);
            frame.direction = along
                .map(|direction| normal(direction).normalize())
                .unwrap_or_else(|| heading(bone));
            frame.offset = above.map_or(Vec3::ZERO, |(tpose, turned, placed)| {
                tpose * turned.inverse() * (place(node) - placed)
            });
            frame.axes = axes(bone, frame.direction, &frame.muscles);
            frames[bone.index()] = frame;
        }
        let scale = mapping
            .node(HumanBone::Hips)
            .map(|hips| place(hips).y)
            .filter(|height| *height > 1e-4)
            .unwrap_or(1.0);
        Self { frames, scale }
    }

    pub(crate) fn shorten_swings(&self, pose: &mut HumanPose) {
        for frame in &self.frames {
            let [Some(bend), Some(side), _] = frame.muscles else {
                continue;
            };
            let (bending, siding) = (muscles()[bend], muscles()[side]);
            let angles = (
                bending.angle(pose.muscles[bend]),
                siding.angle(pose.muscles[side]),
            );
            let swing = angles.0.hypot(angles.1);
            if swing <= LONGEST_SWING {
                continue;
            }
            let scale = LONGEST_SWING / swing;
            pose.muscles[bend] = bending.value(angles.0 * scale);
            pose.muscles[side] = siding.value(angles.1 * scale);
        }
    }

    pub(crate) fn frame(&self, bone: HumanBone) -> &Frame {
        &self.frames[bone.index()]
    }

    pub(crate) fn scale(&self) -> f32 {
        self.scale
    }

    pub(crate) fn axis(&self, bone: HumanBone, dof: Dof) -> Option<Vec3> {
        let frame = self.frame(bone);
        let slot = DOFS.iter().position(|candidate| *candidate == dof)?;
        frame.muscles[slot]?;
        frame.present.then_some(frame.axes[slot])
    }

    pub(crate) fn local_rotation(&self, bone: HumanBone, pose: &HumanPose) -> Quat {
        if bone == HumanBone::Hips {
            return pose.root_rot;
        }
        let frame = self.frame(bone);
        let angle = |slot: usize| {
            frame.muscles[slot].map_or(0.0, |index| muscles()[index].angle(pose.muscles[index]))
        };
        let swing = Quat::from_scaled_axis(frame.axes[0] * angle(0) + frame.axes[1] * angle(1));
        let twist = Quat::from_axis_angle(frame.axes[2], angle(2));
        (swing * twist).normalize()
    }

    pub(crate) fn set_local_rotation(&self, bone: HumanBone, rotation: Quat, pose: &mut HumanPose) {
        if bone == HumanBone::Hips {
            pose.root_rot = rotation.normalize();
            return;
        }
        let frame = self.frame(bone);
        for (slot, angle) in angles(frame, rotation).into_iter().enumerate() {
            if let Some(index) = frame.muscles[slot] {
                pose.muscles[index] = muscles()[index].value(angle);
            }
        }
    }

    pub(crate) fn fk(&self, pose: &HumanPose) -> Worlds {
        let mut worlds = Worlds::default();
        for bone in HumanBone::ALL.iter().copied() {
            let frame = self.frame(bone);
            if !frame.present {
                continue;
            }
            let joint = match frame.parent.and_then(|parent| worlds.get(parent)) {
                Some(above) if bone != HumanBone::Hips => Joint {
                    position: above.point(frame.offset),
                    rotation: (above.rotation * self.local_rotation(bone, pose)).normalize(),
                },
                _ => Joint {
                    position: pose.root_pos * self.scale,
                    rotation: pose.root_rot.normalize(),
                },
            };
            worlds.set(bone, joint);
        }
        worlds
    }
}

fn angles(frame: &Frame, rotation: Quat) -> [f32; 3] {
    let (swing, twist) = split(rotation, frame.axes[2]);
    let bent = swing.to_scaled_axis();
    [bent.dot(frame.axes[0]), bent.dot(frame.axes[1]), twist]
}

fn axes(bone: HumanBone, direction: Vec3, dofs: &[Option<usize>; 3]) -> [Vec3; 3] {
    let nominal =
        |slot: usize, fallback: Vec3| dofs[slot].map_or(fallback, |index| muscles()[index].axis);
    let twisted = nominal(2, heading(bone));
    let twist = match direction.dot(twisted) < 0.0 {
        true => -direction,
        false => direction,
    };
    let flat = |axis: Vec3, against: &[Vec3]| {
        let mut left = axis;
        for other in against {
            left -= *other * left.dot(*other);
        }
        Some(left)
            .filter(|left| left.length() > 1e-3)
            .map(Vec3::normalize)
    };
    let bend = flat(nominal(0, direction.any_orthonormal_vector()), &[twist])
        .unwrap_or_else(|| twist.any_orthonormal_vector());
    let side = flat(nominal(1, twist.cross(bend)), &[twist, bend])
        .unwrap_or_else(|| twist.cross(bend).normalize());
    [bend, side, twist]
}
