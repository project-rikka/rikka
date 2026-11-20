use rikka_model_core::bake::{self, Subdivision, TransformParts, TransformSamples};
use rikka_model_core::glam::{Mat4, Quat, Vec3};
use rikka_model_core::{
    Animation, Channel as Target, ImportCtx, Interpolation, Model, NodeId, Result, Track,
    Transform, Weights,
};
use rustc_hash::FxHashMap;

use super::animation::{Clock, Curves, quoted_name};
use super::assemble::{Assembly, Rig};
use super::modifier::Notes;
use super::transform::{
    Basis, Resize, Rotation, axis_angle, euler, finite, inverse, object_basis, quaternion,
    stored_size,
};
use crate::file::Record;

const LOCATION: usize = 0;
const EULER: usize = 1;
const QUATERNION: usize = 2;
const AXIS_ANGLE: usize = 3;
const SCALE: usize = 4;
const TRANSFORM_TRACKS: usize = 3;
const FRAMES: Subdivision = Subdivision::Grid { step: 1.0 };
const TRANSFORM_PATHS: [&str; 5] = [
    "location",
    "rotation_euler",
    "rotation_quaternion",
    "rotation_axis_angle",
    "scale",
];

struct RestTransform {
    location: Vec3,
    rotation: Rotation,
    scale: Vec3,
}

struct Offset {
    location: Vec3,
    rotation: Quat,
    resize: Resize,
}

impl Offset {
    const NONE: Self = Self {
        location: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        resize: Resize::NONE,
    };
}

struct AnimatedNode {
    node: NodeId,
    paths: [String; 5],
    around: Mat4,
    rest: RestTransform,
    offset: Offset,
}

impl AnimatedNode {
    fn value(
        &self,
        curves: &Curves,
        property: usize,
        index: i64,
        frame: f32,
        fallback: f32,
    ) -> f32 {
        curves.sample(&self.paths[property], index, frame, fallback)
    }

    fn vector(&self, curves: &Curves, property: usize, frame: f32, fallback: Vec3) -> Vec3 {
        Vec3::new(
            self.value(curves, property, 0, frame, fallback.x),
            self.value(curves, property, 1, frame, fallback.y),
            self.value(curves, property, 2, frame, fallback.z),
        )
    }

    fn rotation(&self, curves: &Curves, frame: f32) -> Quat {
        match self.rest.rotation {
            Rotation::Quaternion(rest_quaternion) => quaternion(std::array::from_fn(|slot| {
                self.value(
                    curves,
                    QUATERNION,
                    slot as i64,
                    frame,
                    rest_quaternion[slot],
                )
            })),
            Rotation::AxisAngle { axis, angle } => axis_angle(
                Vec3::new(
                    self.value(curves, AXIS_ANGLE, 1, frame, axis.x),
                    self.value(curves, AXIS_ANGLE, 2, frame, axis.y),
                    self.value(curves, AXIS_ANGLE, 3, frame, axis.z),
                ),
                self.value(curves, AXIS_ANGLE, 0, frame, angle),
            ),
            Rotation::Euler { angles, mode } => {
                euler(self.vector(curves, EULER, frame, angles), mode)
            }
        }
    }

    fn matrix(&self, curves: &Curves, frame: f32) -> Mat4 {
        let basis = Basis {
            location: self.vector(curves, LOCATION, frame, self.rest.location)
                + self.offset.location,
            rotation: self.offset.rotation * self.rotation(curves, frame),
            scale: self
                .offset
                .resize
                .apply(self.vector(curves, SCALE, frame, self.rest.scale)),
        };
        self.around * basis.matrix()
    }
}

impl AnimatedNode {
    fn transform(&self, curves: &Curves, frame: f32) -> Transform {
        Transform::from_matrix(finite(self.matrix(curves, frame)))
    }
}

struct AnimatedObject {
    curves: Curves,
    moving: Vec<AnimatedNode>,
}

struct AnimatedShapes {
    node: NodeId,
    curves: Curves,
    rest_weights: Vec<f32>,
    slots: FxHashMap<String, usize>,
}

impl AnimatedShapes {
    fn weights(&self, frame: f32) -> Vec<f32> {
        let mut weights = self.rest_weights.clone();
        for (path, &slot) in &self.slots {
            weights[slot] = self.curves.sample(path, 0, frame, self.rest_weights[slot]);
        }
        weights
    }

    fn channel(&self, times: Vec<f32>, values: Vec<f32>) -> Target {
        Target {
            target: self.node,
            track: Track::Weights(Weights {
                times,
                values,
                count: self.rest_weights.len(),
                interpolation: Interpolation::Linear,
            }),
        }
    }
}

pub(crate) struct Motion {
    subjects: Vec<AnimatedObject>,
    morphs: Vec<AnimatedShapes>,
    name: String,
}

impl Motion {
    pub(crate) fn resolve(assembly: &Assembly<'_>, model: &Model, notes: &mut Notes) -> Self {
        let mut subjects = Vec::new();
        let mut names = Vec::new();
        for (index, placement) in assembly.stage.placements.iter().enumerate() {
            if placement.instanced {
                continue;
            }
            let Some(node) = assembly.nodes[index] else {
                continue;
            };
            let object = placement.object;
            let Some(curves) = Curves::from_id(&object, notes) else {
                continue;
            };
            if let Some(name) = action_name(&object) {
                names.push(name);
            }
            let mut moving = Vec::new();
            let paths = paths("");
            if moves(&curves, &paths) {
                let mode = object.int("rotmode").unwrap_or(1);
                moving.push(AnimatedNode {
                    node,
                    paths,
                    around: model.nodes[node.index()].transform.to_matrix()
                        * inverse(object_basis(&object).matrix()),
                    rest: RestTransform {
                        location: object.vec3("loc").unwrap_or(Vec3::ZERO),
                        rotation: Rotation::read(&object, mode, false),
                        scale: stored_size(&object),
                    },
                    offset: Offset {
                        location: object.vec3("dloc").unwrap_or(Vec3::ZERO),
                        rotation: Rotation::read(&object, mode, true).quat(),
                        resize: Resize::from_object(&object),
                    },
                });
            }
            if let Some(rig) = assembly.rigs.get(&index) {
                bones(rig, &object, &curves, &mut moving, notes);
            }
            if !moving.is_empty() {
                subjects.push(AnimatedObject { curves, moving });
            }
        }

        let morphs = assembly
            .morphed
            .iter()
            .filter_map(|morphed| {
                let curves = Curves::from_id(&morphed.key, notes)?;
                let mut slots = FxHashMap::default();
                for curve in curves.curves() {
                    let parsed = quoted_name(&curve.path, "key_blocks")
                        .or_else(|| quoted_name(&curve.path, "keys"));
                    if let Some((name, "value")) = parsed
                        && let Some(slot) =
                            morphed.shapes.iter().position(|(shape, _)| *shape == name)
                    {
                        slots.insert(curve.path.clone(), slot);
                    }
                }
                (!slots.is_empty()).then(|| AnimatedShapes {
                    node: morphed.node,
                    rest_weights: morphed.shapes.iter().map(|(_, weight)| *weight).collect(),
                    curves,
                    slots,
                })
            })
            .collect();

        names.dedup();
        let name = match names.as_slice() {
            [single] => single.clone(),
            _ => assembly
                .stage
                .scene
                .map(|scene| scene.id_name())
                .unwrap_or_else(|| "Animation".to_owned()),
        };
        Self {
            subjects,
            morphs,
            name,
        }
    }

    pub(crate) fn bake(&self, clock: &Clock, ctx: &mut ImportCtx<'_>) -> Result<Option<Animation>> {
        let (from, to) = (clock.start as f32, clock.end as f32);
        let mut channels = Vec::new();
        for subject in &self.subjects {
            for moving in &subject.moving {
                ctx.checkpoint()?;
                let wanted = |path: &str| moving.paths.iter().any(|wanted| wanted == path);
                let seeds = subject.curves.breaks(&wanted, from, to);
                let baked = bake::adaptive(seeds, FRAMES, |frame| {
                    moving.transform(&subject.curves, frame as f32)
                });
                ctx.charge_keyframes(baked.len() * TRANSFORM_TRACKS)?;
                let mut sampled = TransformSamples::with_capacity(baked.len());
                for (frame, transform) in baked {
                    sampled.push(clock.time(frame as f32), continued(&sampled, transform));
                }
                channels.extend(sampled.channels(moving.node, TransformParts::ALL));
            }
        }
        for morph in &self.morphs {
            ctx.checkpoint()?;
            let wanted = |path: &str| morph.slots.contains_key(path);
            let seeds = morph.curves.breaks(&wanted, from, to);
            let baked = bake::adaptive(seeds, FRAMES, |frame| morph.weights(frame as f32));
            ctx.charge_keyframes(baked.len() * morph.rest_weights.len().max(1))?;
            let (times, values): (Vec<f32>, Vec<Vec<f32>>) = baked
                .into_iter()
                .map(|(frame, weights)| (clock.time(frame as f32), weights))
                .unzip();
            channels.push(morph.channel(times, values.concat()));
        }
        Ok(self.animation(channels))
    }

    pub(crate) fn still(&self, frame: f32) -> Option<Animation> {
        let mut channels = Vec::new();
        for subject in &self.subjects {
            for moving in &subject.moving {
                let mut sampled = TransformSamples::with_capacity(1);
                sampled.push(0.0, moving.transform(&subject.curves, frame));
                channels.extend(sampled.channels(moving.node, TransformParts::ALL));
            }
        }
        for morph in &self.morphs {
            channels.push(morph.channel(vec![0.0], morph.weights(frame)));
        }
        self.animation(channels)
    }

    fn animation(&self, channels: Vec<Target>) -> Option<Animation> {
        if channels.is_empty() {
            return None;
        }
        let mut animation = Animation::new(self.name.clone());
        animation.channels = channels;
        Some(animation)
    }
}

fn continued(sampled: &TransformSamples, mut transform: Transform) -> Transform {
    if let Some(previous) = sampled.last_rotation()
        && previous.dot(transform.rotation) < 0.0
    {
        transform.rotation = -transform.rotation;
    }
    transform
}

fn paths(prefix: &str) -> [String; 5] {
    TRANSFORM_PATHS.map(|property| format!("{prefix}{property}"))
}

fn moves(curves: &Curves, paths: &[String; 5]) -> bool {
    curves.curves().any(|curve| paths.contains(&curve.path))
}

fn action_name(object: &Record<'_>) -> Option<String> {
    Some(object.deref("adt")?.id("action")?.id_name())
}

fn bones(
    rig: &Rig,
    object: &Record<'_>,
    curves: &Curves,
    moving: &mut Vec<AnimatedNode>,
    notes: &mut Notes,
) {
    if rig.channels.iter().any(|channel| channel.constrained) {
        notes.unsupported(format!(
            "bone constraints of \"{}\" not evaluated during animation",
            object.id_name()
        ));
    }
    let mut animated = vec![false; rig.skeleton.bones.len()];
    for curve in curves.curves() {
        if let Some((name, property)) = quoted_name(&curve.path, "pose.bones")
            && TRANSFORM_PATHS.contains(&property)
            && let Some(bone) = rig.skeleton.find(&name)
        {
            animated[bone] = true;
        }
    }
    for (bone, moves) in animated.into_iter().enumerate() {
        if !moves {
            continue;
        }
        let name = &rig.skeleton.bones[bone].name;
        if !rig.skeleton.bones[bone].plain {
            notes.unsupported(format!(
                "bone \"{name}\" of \"{}\": partial transform inheritance not evaluated in animation",
                object.id_name()
            ));
        }
        let channel = &rig.channels[bone];
        moving.push(AnimatedNode {
            node: rig.joints[bone],
            paths: paths(&format!("pose.bones[\"{}\"].", escape(name))),
            around: rig.skeleton.rest_local(bone),
            rest: RestTransform {
                location: channel.location,
                rotation: channel.rotation,
                scale: channel.scale,
            },
            offset: Offset::NONE,
        });
    }
}

fn escape(name: &str) -> String {
    name.replace('\\', "\\\\").replace('"', "\\\"")
}
