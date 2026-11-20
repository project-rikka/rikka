use std::borrow::Cow;

use rikka_model_core::glam::{Quat, Vec3};
use rustc_hash::FxHashMap;

use rikka_model_core::ExportCtx;
use rikka_model_core::Model;
use rikka_model_core::NodeId;
use rikka_model_core::Stage;
use rikka_model_core::Warning;
use rikka_model_core::{Curve, Track, Weights};
use rikka_model_core::{Error, Requirement, Result};

use super::super::text::encode_name;
use super::super::{in_mmd_space, model_name};
use super::document::{
    FRAME_NAME, FRAMES_PER_SECOND, HEADER, IK_NAME, NAME, PHYSICS_OFF, SIGNATURE, Vmd,
    VmdBoneFrame, VmdInterpolation, VmdMorphFrame,
};
use rikka_model_core::ByteWriter;

pub(crate) fn write(model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<u8>> {
    let model = in_mmd_space(model);
    let motion = match model.extensions.get::<Vmd>() {
        Some(vmd) => Cow::Borrowed(vmd),
        None => Cow::Owned(motion_of(&model, ctx)?),
    };
    ctx.report(Stage::Writing, 0.5);
    Ok(emit(&motion))
}

fn emit(motion: &Vmd) -> Vec<u8> {
    let mut writer = ByteWriter::with_capacity(HEADER + NAME + motion.bones.len() * 111);
    let mut names = Names::default();
    writer.fixed(SIGNATURE, HEADER);
    writer.fixed(names.encode(&motion.model), NAME);

    writer.u32(motion.bones.len() as u32);
    for frame in &motion.bones {
        writer.fixed(names.encode(&frame.bone), FRAME_NAME);
        writer.u32(frame.frame);
        writer.vec3(frame.translation);
        writer.vec4(rikka_model_core::glam::Vec4::from(frame.rotation));
        writer.bytes(&interpolation_bytes(frame.interpolation, frame.physics_off));
    }

    writer.u32(motion.morphs.len() as u32);
    for frame in &motion.morphs {
        writer.fixed(names.encode(&frame.morph), FRAME_NAME);
        writer.u32(frame.frame);
        writer.f32(frame.weight);
    }

    writer.u32(motion.cameras.len() as u32);
    for frame in &motion.cameras {
        writer.u32(frame.frame);
        writer.f32(frame.distance);
        writer.vec3(frame.target);
        writer.vec3(frame.rotation);
        for curve in frame.interpolation.curves() {
            let curve_bytes = curve.to_bytes();
            writer.bytes(&[
                curve_bytes[0],
                curve_bytes[2],
                curve_bytes[1],
                curve_bytes[3],
            ]);
        }
        writer.u32(frame.angle);
        writer.u8(u8::from(!frame.perspective));
    }

    writer.u32(motion.lights.len() as u32);
    for frame in &motion.lights {
        writer.u32(frame.frame);
        writer.vec3(frame.color);
        writer.vec3(frame.direction);
    }

    writer.u32(motion.shadows.len() as u32);
    for frame in &motion.shadows {
        writer.u32(frame.frame);
        writer.u8(frame.mode);
        writer.f32(frame.distance);
    }

    writer.u32(motion.properties.len() as u32);
    for frame in &motion.properties {
        writer.u32(frame.frame);
        writer.u8(u8::from(frame.visible));
        writer.u32(frame.ik.len() as u32);
        for (bone, enabled) in &frame.ik {
            writer.fixed(names.encode(bone), IK_NAME);
            writer.u8(u8::from(*enabled));
        }
    }
    writer.finish()
}

#[derive(Default)]
struct Names {
    cache: FxHashMap<String, Vec<u8>>,
}

impl Names {
    fn encode(&mut self, name: &str) -> &[u8] {
        self.cache
            .entry(name.to_owned())
            .or_insert_with(|| encode_name(name))
    }
}
fn interpolation_bytes(interpolation: VmdInterpolation, physics_off: bool) -> [u8; 64] {
    let mut interleaved = [0_u8; 19];
    for (curve, values) in interpolation.curves().iter().enumerate() {
        for (parameter, value) in values.to_bytes().into_iter().enumerate() {
            interleaved[parameter * 4 + curve] = value;
        }
    }
    let mut bytes = [0_u8; 64];
    for shift in 0..4 {
        for group in 0..4 {
            let first = group * 4 + shift;
            let at = (shift * 4 + group) * 4;
            bytes[at..at + 4].copy_from_slice(&interleaved[first..first + 4]);
        }
    }
    if physics_off {
        bytes[2..4].copy_from_slice(&PHYSICS_OFF.to_le_bytes());
    }
    bytes
}

fn motion_of(model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vmd> {
    let Some(animation) = model.animations.first() else {
        return Err(Error::Unmet {
            format: "VMD",
            requirement: Requirement::Animation,
        });
    };
    if model.animations.len() > 1 {
        ctx.warn(Warning::Unsupported(format!(
            "VMD holds one animation: writing \"{}\", dropping {} others",
            animation.name,
            model.animations.len() - 1
        )));
    }

    let mut motion = Vmd {
        model: model_name(model),
        ..Vmd::default()
    };
    let mut turned: FxHashMap<NodeId, Bone<'_>> = FxHashMap::default();
    for channel in &animation.channels {
        let bone = turned.entry(channel.target).or_insert_with(|| Bone {
            name: model.node(channel.target).name.as_str(),
            rest: model.node(channel.target).transform.translation,
            frames: Vec::new(),
            translation: None,
            rotation: None,
        });
        match &channel.track {
            Track::Translation(curve) => {
                bone.frames.extend(curve.times.iter().copied());
                bone.translation = Some(curve);
            }
            Track::Rotation(curve) => {
                bone.frames.extend(curve.times.iter().copied());
                bone.rotation = Some(curve);
            }
            Track::Scale(_) => {}
            Track::Weights(weights) => write_morphs(model, channel.target, weights, &mut motion),
        }
    }

    for bone in turned.values_mut() {
        bone.frames.sort_by(f32::total_cmp);
        bone.frames.dedup();
        for time in &bone.frames {
            let translation = bone
                .translation
                .and_then(|curve| curve.sample(*time))
                .unwrap_or(bone.rest);
            motion.bones.push(VmdBoneFrame {
                bone: bone.name.to_owned(),
                frame: frame_of(*time),
                translation: translation - bone.rest,
                rotation: bone
                    .rotation
                    .and_then(|curve| curve.sample(*time))
                    .unwrap_or(Quat::IDENTITY),
                interpolation: VmdInterpolation::default(),
                physics_off: false,
            });
        }
    }
    motion.sort();
    Ok(motion)
}

struct Bone<'a> {
    name: &'a str,
    rest: Vec3,
    frames: Vec<f32>,
    translation: Option<&'a Curve<Vec3>>,
    rotation: Option<&'a Curve<Quat>>,
}

fn write_morphs(model: &Model, node: NodeId, weights: &Weights, motion: &mut Vmd) {
    let Some(mesh) = model.node(node).mesh else {
        return;
    };
    let targets = &model.mesh(mesh).morph_targets;
    for (index, time) in weights.times.iter().copied().enumerate() {
        let Some(key_weights) = weights.key(index) else {
            continue;
        };
        for (target, weight) in targets.iter().zip(key_weights) {
            motion.morphs.push(VmdMorphFrame {
                morph: target.name.clone(),
                frame: frame_of(time),
                weight: *weight,
            });
        }
    }
}

fn frame_of(time: f32) -> u32 {
    (time * FRAMES_PER_SECOND).round().max(0.0) as u32
}
