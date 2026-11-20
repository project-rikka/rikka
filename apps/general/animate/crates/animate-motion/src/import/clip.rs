use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use animate_body::{Body, HumanBone, HumanPose};
use rikka_humanoid::Anatomy;
use rikka_mmd::{VmdBoneFrame, VmdCurve, VmdInterpolation};
use rikka_model_core::glam::{EulerRot, Quat};
use rikka_model_core::{Animation, Instance, Model, NodeId, Track};
use rikka_vrm::{AnimatedExpression, VrmAnimation};
use rustc_hash::FxHashSet;

use super::adopting::Adopting;
use super::{Recording, Source, Target, human_motion, node_names};
use crate::morphs::{MorphKey, Morphs};
use crate::take::Take;
use crate::tuning::FRAMES_PER_SECOND;

#[derive(Debug)]
pub struct Clip {
    model: Arc<Model>,
    animation: usize,
    body: Option<Arc<Body>>,
}

impl Clip {
    #[must_use]
    pub fn new(model: Arc<Model>, animation: usize) -> Option<Self> {
        let body = Body::from_anatomy(&model, &Anatomy::recognize(&model))
            .ok()
            .map(Arc::new);
        Self::with_body(model, animation, body)
    }

    #[must_use]
    pub fn with_body(model: Arc<Model>, animation: usize, body: Option<Arc<Body>>) -> Option<Self> {
        model.animations.get(animation)?;
        Some(Self {
            model,
            animation,
            body,
        })
    }

    #[must_use]
    pub fn is_human(&self) -> bool {
        self.body.is_some()
    }

    fn animation(&self) -> &Animation {
        &self.model.animations[self.animation]
    }
}

struct Detail<'a> {
    source: NodeId,
    name: &'a str,
    frames: &'a BTreeSet<u32>,
}

struct Weighted<'a> {
    node: NodeId,
    names: Vec<&'a str>,
    frames: BTreeSet<u32>,
}

struct Expressed<'a> {
    expression: &'a AnimatedExpression,
    frames: &'a BTreeSet<u32>,
}

fn frame_of(seconds: f32) -> u32 {
    (seconds.max(0.0) * FRAMES_PER_SECOND).round() as u32
}

impl Take {
    #[must_use]
    pub fn from_clip(target: Target<'_>, clip: &Clip) -> Self {
        let model = &clip.model;
        let animation = clip.animation();
        let vrma = model.extensions.get::<VrmAnimation>();
        let mut moved: BTreeMap<NodeId, BTreeSet<u32>> = BTreeMap::new();
        let mut weighted: BTreeMap<NodeId, BTreeSet<u32>> = BTreeMap::new();
        for channel in &animation.channels {
            let (times, frames) = match &channel.track {
                Track::Translation(curve) | Track::Scale(curve) => (&curve.times, &mut moved),
                Track::Rotation(curve) => (&curve.times, &mut moved),
                Track::Weights(weights) => (&weights.times, &mut weighted),
            };
            frames
                .entry(channel.target)
                .or_default()
                .extend(times.iter().copied().map(frame_of));
        }

        let signals: FxHashSet<NodeId> = vrma
            .iter()
            .flat_map(|vrma| {
                let expressions = vrma.expressions.iter().map(|expression| expression.node);
                expressions.chain(vrma.look_at.and_then(|look_at| look_at.node))
            })
            .collect();
        let expressed: Vec<Expressed<'_>> = vrma
            .iter()
            .flat_map(|vrma| &vrma.expressions)
            .filter_map(|expression| {
                Some(Expressed {
                    expression,
                    frames: moved.get(&expression.node)?,
                })
            })
            .collect();
        let targets = node_names(target.model);
        let source_drives = |node: NodeId| {
            clip.body
                .as_ref()
                .is_some_and(|body| body.mapping().is_driven(node))
        };
        let details: Vec<Detail<'_>> = moved
            .iter()
            .filter(|(node, _)| !signals.contains(node) && !source_drives(**node))
            .filter_map(|(node, frames)| {
                let name = model.node(*node).name.as_str();
                let target_node = targets.get(name)?;
                (!target.drives(*target_node)).then_some(Detail {
                    source: *node,
                    name,
                    frames,
                })
            })
            .collect();
        let weighted: Vec<Weighted<'_>> = weighted
            .into_iter()
            .filter_map(|(node, frames)| {
                let mesh = model.mesh(model.node(node).mesh?);
                Some(Weighted {
                    node,
                    names: mesh
                        .morph_targets
                        .iter()
                        .map(|target| target.name.as_str())
                        .collect(),
                    frames,
                })
            })
            .collect();

        let last = frame_of(animation.duration());
        let mut instance = Instance::new(Arc::clone(model));
        let mut poses: Vec<HumanPose> = Vec::new();
        let mut bones = Vec::new();
        let mut weights: Vec<MorphKey> = Vec::new();
        for frame in 0..=last {
            instance.pose_mut().reset(model);
            animation.apply(frame as f32 / FRAMES_PER_SECOND, instance.pose_mut());
            if let Some(body) = &clip.body {
                instance.refresh();
                let mut pose = body.read(&instance);
                if let (Some(gaze), Some(target_body)) = (
                    vrma.and_then(|vrma| vrma.gaze(instance.pose())),
                    target.body,
                ) {
                    let turn = Quat::from_euler(EulerRot::YXZ, gaze.yaw, gaze.pitch, 0.0);
                    for eye in [HumanBone::LeftEye, HumanBone::RightEye] {
                        target_body.set_local_rotation(eye, turn, &mut pose);
                    }
                }
                poses.push(pose);
            }
            let pose = instance.pose();
            let key = |morph: &str, weight: f32| MorphKey {
                morph: morph.to_owned(),
                frame,
                weight,
                curve: VmdCurve::LINEAR,
            };
            for detail in details
                .iter()
                .filter(|detail| detail.frames.contains(&frame))
            {
                let rest = model.node(detail.source).transform;
                let local = pose.local(detail.source);
                bones.push(VmdBoneFrame {
                    bone: detail.name.to_owned(),
                    frame,
                    translation: local.translation - rest.translation,
                    rotation: (rest.rotation.inverse() * local.rotation).normalize(),
                    interpolation: VmdInterpolation::default(),
                    physics_off: false,
                });
            }
            for source in weighted
                .iter()
                .filter(|source| source.frames.contains(&frame))
            {
                for (name, weight) in source.names.iter().zip(pose.weights(source.node)) {
                    weights.push(key(name, *weight));
                }
            }
            for expressed in expressed
                .iter()
                .filter(|expressed| expressed.frames.contains(&frame))
            {
                let expression = expressed.expression;
                weights.push(key(expression.name.as_str(), expression.weight(pose)));
            }
        }

        let keyed: Vec<(NodeId, u32)> = moved
            .iter()
            .flat_map(|(node, frames)| frames.iter().map(|frame| (*node, *frame)))
            .collect();
        let human = clip
            .body
            .as_deref()
            .zip(target.body)
            .and_then(|(body, target_body)| {
                let source = Source { model, body };
                human_motion(source, target_body, &Recording { poses, keyed })
            });
        Take {
            human,
            bones: bones.into_iter().collect(),
            morphs: morphs(target, weights),
            ..Take::default()
        }
    }
}

fn morphs(target: Target<'_>, weights: Vec<MorphKey>) -> Morphs {
    let moving: FxHashSet<String> = weights
        .iter()
        .filter(|key| key.weight != 0.0)
        .map(|key| key.morph.clone())
        .collect();
    let mut adopting = Adopting::new(target.faces);
    let mut morphs = Morphs::default();
    for key in weights
        .into_iter()
        .filter(|key| moving.contains(&key.morph))
    {
        for adopted in adopting.keys(key) {
            morphs.insert(adopted);
        }
    }
    morphs
}
