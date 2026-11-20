use rikka_model_core::glam::{Mat4, Vec3};
use rustc_hash::FxHashMap;

use super::transform::{Basis, Rotation, finite, inverse, stored_size};
use crate::file::Record;

const BONE_NO_DEFORM: i64 = 1 << 12;
const BONE_HINGE: i64 = 1 << 9;
const BONE_NO_LOCAL_LOCATION: i64 = 1 << 22;
const BONE_RELATIVE_PARENTING: i64 = 1 << 23;
const INHERIT_SCALE_FULL: i64 = 0;

#[derive(Clone, Debug)]
pub(crate) struct Bone {
    pub(crate) name: String,
    pub(crate) parent: Option<usize>,
    pub(crate) rest: Mat4,
    pub(crate) length: f32,
    pub(crate) deform: bool,
    pub(crate) plain: bool,
    pub(crate) relative_parent: bool,
    pub(crate) segments: i64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Skeleton {
    pub(crate) bones: Vec<Bone>,
    by_name: FxHashMap<String, usize>,
}

impl Skeleton {
    pub(crate) fn read(armature: Record<'_>) -> Self {
        let mut skeleton = Self::default();
        for root in armature.list("bonebase") {
            skeleton.visit(root, None, 0);
        }
        skeleton
    }

    fn visit(&mut self, bone: Record<'_>, parent: Option<usize>, depth: usize) {
        if depth > 512 {
            return;
        }
        let flag = bone.int("flag").unwrap_or(0);
        let inherit = bone.int("inherit_scale_mode").unwrap_or(INHERIT_SCALE_FULL);
        let name = bone.text("name").unwrap_or_default();
        let index = self.bones.len();
        self.bones.push(Bone {
            name: name.clone(),
            parent,
            rest: finite(bone.matrix("arm_mat").unwrap_or(Mat4::IDENTITY)),
            length: bone.float("length").unwrap_or(0.0),
            deform: flag & BONE_NO_DEFORM == 0,
            plain: flag & (BONE_HINGE | BONE_NO_LOCAL_LOCATION) == 0
                && inherit == INHERIT_SCALE_FULL,
            relative_parent: flag & BONE_RELATIVE_PARENTING != 0,
            segments: bone.int("segments").unwrap_or(1),
        });
        self.by_name.entry(name).or_insert(index);
        for child in bone.list("childbase") {
            self.visit(child, Some(index), depth + 1);
        }
    }

    pub(crate) fn find(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    pub(crate) fn rest_local(&self, bone: usize) -> Mat4 {
        let own = self.bones[bone].rest;
        match self.bones[bone].parent {
            Some(parent) => inverse(self.bones[parent].rest) * own,
            None => own,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Channel {
    pub(crate) location: Vec3,
    pub(crate) rotation: Rotation,
    pub(crate) scale: Vec3,
    pub(crate) pose_matrix: Option<Mat4>,
    pub(crate) constrained: bool,
}

impl Channel {
    pub(crate) fn basis(&self) -> Basis {
        Basis {
            location: self.location,
            rotation: self.rotation.quat(),
            scale: self.scale,
        }
    }
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            location: Vec3::ZERO,
            rotation: Rotation::IDENTITY,
            scale: Vec3::ONE,
            pose_matrix: None,
            constrained: false,
        }
    }
}

pub(crate) fn channels(object: Record<'_>, skeleton: &Skeleton) -> Vec<Channel> {
    let mut channels = vec![Channel::default(); skeleton.bones.len()];
    let Some(pose) = object.deref("pose") else {
        return channels;
    };
    for channel in pose.list("chanbase") {
        let Some(bone) = skeleton.find(&channel.text("name").unwrap_or_default()) else {
            continue;
        };
        let pose_matrix = channel
            .matrix("pose_mat")
            .filter(|matrix| matrix.is_finite() && matrix.determinant().abs() > 1.0e-12);
        channels[bone] = Channel {
            location: channel.vec3("loc").unwrap_or(Vec3::ZERO),
            rotation: Rotation::read(&channel, channel.int("rotmode").unwrap_or(0), false),
            scale: stored_size(&channel),
            pose_matrix,
            constrained: !channel.list("constraints").is_empty(),
        };
    }
    channels
}

pub(crate) fn pose(skeleton: &Skeleton, locals: &[Mat4]) -> Vec<Mat4> {
    let mut posed: Vec<Mat4> = Vec::with_capacity(skeleton.bones.len());
    for (index, bone) in skeleton.bones.iter().enumerate() {
        let local = skeleton.rest_local(index) * locals[index];
        posed.push(match bone.parent {
            Some(parent) => posed[parent] * local,
            None => local,
        });
    }
    posed
}

pub(crate) fn current(skeleton: &Skeleton, channels: &[Channel]) -> Vec<Mat4> {
    if let Some(pose_matrices) = channels
        .iter()
        .map(|channel| channel.pose_matrix)
        .collect::<Option<Vec<_>>>()
    {
        return pose_matrices;
    }
    let locals: Vec<Mat4> = channels
        .iter()
        .map(|channel| channel.basis().matrix())
        .collect();
    pose(skeleton, &locals)
}
