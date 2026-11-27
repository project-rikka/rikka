use rikka_model_core::glam::{Quat, Vec3};
use rustc_hash::{FxHashMap, FxHashSet};

use rikka_model_core::CoordinateSystem;
use rikka_model_core::ImportCtx;
use rikka_model_core::Model;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Result;
use rikka_model_core::Transform;
use rikka_model_core::bake::{self, Subdivision, TransformParts, TransformSamples};
use rikka_model_core::{Animation, Channel, Curve, Interpolation, Track, Weights};
use rikka_model_core::{Camera, Projection};
use rikka_model_core::{Light, LightKind};
use rikka_model_core::{Node, NodeId};

use super::super::MMD_UNIT_SCALE;
use super::PARALLEL;
use super::bones::pose;
use super::camera::VmdCamera;
use super::document::{FRAMES_PER_SECOND, Vmd, VmdBoneFrame, VmdMorphFrame};

const CAMERA: &str = "camera";
const LIGHT: &str = "light";
const CAMERA_NEAR: f32 = 0.05;

const CUT: f32 = 1e-4;
const FRAMES: Subdivision = Subdivision::Grid { step: 1.0 };
const TRACKS_PER_WORKER: usize = 4;

pub(crate) fn model_of(motion: Vmd, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let mut builder = ModelBuilder::new()
        .coordinates(CoordinateSystem::LEFT_HANDED_Y_UP)
        .unit_scale(MMD_UNIT_SCALE)
        .generator(motion.model.clone());

    let mut animation = Animation::new(match motion.model.is_empty() {
        true => "motion".to_owned(),
        false => motion.model.clone(),
    });
    let bones = motion.bones_by_name();
    let sampled = sampled_keyframes(&motion, &bones);
    ctx.charge_keyframes(usize::try_from(sampled).unwrap_or(usize::MAX))?;
    let tracks: Vec<(NodeId, Vec<&VmdBoneFrame>)> = bones
        .into_iter()
        .map(|(bone, frames)| (builder.add_node(Node::new(bone)), frames))
        .collect();
    animation
        .channels
        .extend(baked_tracks(&tracks, |_| Vec3::ZERO));
    for (morph, frames) in motion.morphs_by_name() {
        let node = builder.add_node(Node::new(morph).morph_weights(vec![0.0]));
        animation.channels.push(Channel {
            target: node,
            track: Track::Weights(Weights {
                times: frames.iter().map(|frame| seconds(frame.frame)).collect(),
                values: frames.iter().map(|frame| frame.weight).collect(),
                count: 1,
                interpolation: Interpolation::Linear,
            }),
        });
    }

    if let Some(camera) = motion.camera() {
        let lens = builder.add_camera(Camera::new(CAMERA).projection(Projection::Perspective {
            y_fov: camera.at(0.0).fov,
            aspect: None,
            near: CAMERA_NEAR / MMD_UNIT_SCALE,
            far: None,
        }));
        let node = builder.add_node(Node::new(CAMERA).camera(lens));
        animation.channels.extend(camera_channels(&camera, node));
    }
    if let Some(first) = motion.lights.first() {
        let light = builder.add_light(
            Light::new(LIGHT, LightKind::Directional).color(first.color.max(Vec3::ZERO)),
        );
        let node = builder.add_node(Node::new(LIGHT).light(light));
        animation.channels.push(Channel {
            target: node,
            track: Track::Rotation(Curve::linear(
                motion
                    .lights
                    .iter()
                    .map(|frame| seconds(frame.frame))
                    .collect(),
                motion
                    .lights
                    .iter()
                    .map(|frame| light_rotation(frame.direction))
                    .collect(),
            )),
        });
    }

    if !animation.channels.is_empty() {
        builder.add_animation(animation);
    }
    builder.extend_with(motion);
    builder.finish()
}

impl Vmd {
    #[must_use]
    pub fn animation(&self, model: &Model) -> Animation {
        let mut animation = Animation::new(match self.model.is_empty() {
            true => "motion".to_owned(),
            false => self.model.clone(),
        });
        let nodes: FxHashMap<&str, NodeId> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.name.as_str(), NodeId(index as u32)))
            .collect();

        let tracks: Vec<(NodeId, Vec<&VmdBoneFrame>)> = self
            .bones_by_name()
            .into_iter()
            .filter_map(|(bone, frames)| Some((nodes.get(bone).copied()?, frames)))
            .collect();
        animation.channels.extend(baked_tracks(&tracks, |node| {
            model.node(node).transform.translation
        }));

        let morphs: FxHashMap<&str, Vec<&VmdMorphFrame>> =
            self.morphs_by_name().into_iter().collect();
        for (node, targets) in morph_targets(model) {
            let tracks: Vec<(usize, &Vec<&VmdMorphFrame>)> = targets
                .iter()
                .enumerate()
                .filter_map(|(index, name)| Some((index, morphs.get(name.as_str())?)))
                .collect();
            if tracks.is_empty() {
                continue;
            }
            let mut times: Vec<u32> = tracks
                .iter()
                .flat_map(|(_, frames)| frames.iter().map(|frame| frame.frame))
                .collect();
            times.sort_unstable();
            times.dedup();

            let mut values = vec![0.0; times.len() * targets.len()];
            for (at, time) in times.iter().enumerate() {
                for (index, frames) in &tracks {
                    values[at * targets.len() + index] = weight_at(frames, *time);
                }
            }
            animation.channels.push(Channel {
                target: node,
                track: Track::Weights(Weights {
                    times: times.iter().map(|frame| seconds(*frame)).collect(),
                    values,
                    count: targets.len(),
                    interpolation: Interpolation::Linear,
                }),
            });
        }
        animation
    }
}

fn morph_targets(model: &Model) -> Vec<(NodeId, Vec<String>)> {
    let mut targets_by_node = Vec::new();
    model.visit(|node| {
        let Some(mesh) = model.node(node).mesh else {
            return;
        };
        let targets = &model.mesh(mesh).morph_targets;
        if !targets.is_empty() {
            targets_by_node.push((
                node,
                targets.iter().map(|target| target.name.clone()).collect(),
            ));
        }
    });
    targets_by_node
}

fn weight_at(frames: &[&super::document::VmdMorphFrame], frame: u32) -> f32 {
    let next = frames.partition_point(|keyframe| keyframe.frame <= frame);
    let Some(before) = next.checked_sub(1) else {
        return frames.first().map_or(0.0, |frame| frame.weight);
    };
    let before = frames[before];
    if before.frame == frame {
        return before.weight;
    }
    match frames.get(next) {
        Some(after) => {
            let span = (after.frame - before.frame) as f32;
            let factor = (frame - before.frame) as f32 / span;
            before.weight + (after.weight - before.weight) * factor
        }
        None => before.weight,
    }
}

fn baked_tracks(
    tracks: &[(NodeId, Vec<&VmdBoneFrame>)],
    rest: impl Fn(NodeId) -> Vec3 + Sync,
) -> Vec<Channel> {
    PARALLEL
        .grain(TRACKS_PER_WORKER)
        .map(tracks, |(node, frames)| {
            bone_channels(frames, *node, rest(*node))
        })
        .into_iter()
        .flatten()
        .collect()
}

fn bone_channels(frames: &[&VmdBoneFrame], node: NodeId, rest: Vec3) -> Vec<Channel> {
    if frames.is_empty() {
        return Vec::new();
    }
    let keys = frames.iter().map(|frame| f64::from(frame.frame)).collect();
    let baked = bake::adaptive(keys, FRAMES, |frame| {
        let (translation, rotation) = pose(frames, frame as f32);
        Transform {
            translation: rest + translation,
            rotation,
            scale: Vec3::ONE,
        }
    });
    let mut samples = TransformSamples::with_capacity(baked.len());
    for (frame, transform) in baked {
        samples.push(frame as f32 / FRAMES_PER_SECOND, transform);
    }
    samples.channels(node, TransformParts::PLACEMENT)
}

fn bent_frames(from: &VmdBoneFrame, to: &VmdBoneFrame) -> u32 {
    match to.interpolation.is_linear() {
        true => 0,
        false => to.frame.saturating_sub(from.frame).saturating_sub(1),
    }
}

fn sampled_keyframes(motion: &Vmd, bones: &[(&str, Vec<&VmdBoneFrame>)]) -> u64 {
    let bones: u64 = bones
        .iter()
        .map(|(_, frames)| {
            let between: u64 = frames
                .windows(2)
                .map(|pair| u64::from(bent_frames(pair[0], pair[1])))
                .sum();
            frames.len() as u64 + between
        })
        .sum();
    let camera = match (
        motion.cameras.iter().map(|frame| frame.frame).min(),
        motion.cameras.iter().map(|frame| frame.frame).max(),
    ) {
        (Some(first), Some(last)) => u64::from(last - first) + 1 + motion.cameras.len() as u64,
        _ => 0,
    };
    bones + camera + motion.morphs.len() as u64 + motion.lights.len() as u64
}

pub(super) fn bent(from: &VmdBoneFrame, to: &VmdBoneFrame, factor: f32) -> Vec3 {
    let curves = [to.interpolation.x, to.interpolation.y, to.interpolation.z];
    let mut translation = Vec3::ZERO;
    for (axis, curve) in curves.iter().enumerate() {
        let factor = curve.factor(factor);
        translation[axis] =
            from.translation[axis] + (to.translation[axis] - from.translation[axis]) * factor;
    }
    translation
}

pub(super) fn turned(from: &VmdBoneFrame, to: &VmdBoneFrame, factor: f32) -> Quat {
    from.rotation
        .slerp(to.rotation, to.interpolation.rotation.factor(factor))
}

fn camera_channels(camera: &VmdCamera, node: NodeId) -> Vec<Channel> {
    let frames = camera.frames();
    let cuts: FxHashSet<u32> = frames
        .windows(2)
        .filter(|pair| pair[1].frame.checked_sub(pair[0].frame) == Some(1))
        .map(|pair| pair[1].frame)
        .collect();
    let placed = |time: f32| {
        let pose = camera.at(time);
        Transform {
            translation: pose.target + pose.rotation() * Vec3::new(0.0, 0.0, pose.distance),
            rotation: pose.rotation(),
            scale: Vec3::ONE,
        }
    };

    let keys = frames.iter().map(|frame| f64::from(frame.frame)).collect();
    let baked = bake::adaptive(keys, FRAMES, |frame| placed(seconds(frame as u32)));
    let mut samples = TransformSamples::with_capacity(baked.len() + cuts.len());
    for (frame, transform) in baked {
        let frame = frame as u32;
        let time = seconds(frame);
        if cuts.contains(&frame) {
            samples.push(time - CUT, placed(seconds(frame - 1)));
        }
        samples.push(time, transform);
    }
    samples.channels(node, TransformParts::PLACEMENT)
}

fn light_rotation(direction: Vec3) -> Quat {
    let direction = direction.normalize_or(Vec3::NEG_Y);
    Quat::from_rotation_arc(Vec3::NEG_Z, direction)
}

fn seconds(frame: u32) -> f32 {
    frame as f32 / FRAMES_PER_SECOND
}
