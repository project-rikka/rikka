use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use animate_body::{Body, HumanBone, HumanPose};
use rikka_humanoid::Anatomy;
use rikka_mmd::{VmdBoneFrame, VmdCurve, VmdInterpolation};
use rikka_model_core::glam::{EulerRot, Quat};
use rikka_model_core::{Animation, Instance, Model, NodeId, Pose, Track};
use rikka_vrm::{AnimatedExpression, VrmAnimation};
use rustc_hash::FxHashSet;

use super::adopting::Adopting;
use super::{FRAMES_PER_WORKER, Recording, Source, Target, human_motion, node_names};
use crate::PARALLEL;
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

    pub(crate) fn model(&self) -> &Arc<Model> {
        &self.model
    }

    pub(crate) fn body(&self) -> Option<&Arc<Body>> {
        self.body.as_ref()
    }

    pub(crate) fn index(&self) -> usize {
        self.animation
    }

    pub(crate) fn animation(&self) -> &Animation {
        &self.model.animations[self.animation]
    }
}

struct Detail<'a> {
    source: NodeId,
    name: &'a str,
}

struct Weighted<'a> {
    node: NodeId,
    names: Vec<&'a str>,
    frames: BTreeSet<u32>,
}

#[derive(Default)]
struct Sampled {
    poses: Vec<HumanPose>,
    bones: Vec<VmdBoneFrame>,
    weights: Vec<MorphKey>,
}

#[derive(Clone, Copy)]
enum Sampling<'b> {
    Keys,
    Poses { target: Option<&'b Body> },
}

struct Plan<'a> {
    clip: &'a Clip,
    vrma: Option<&'a VrmAnimation>,
    moved: BTreeMap<NodeId, BTreeSet<u32>>,
    details: Vec<Detail<'a>>,
    weighted: Vec<Weighted<'a>>,
    expressed: Vec<&'a AnimatedExpression>,
}

pub(crate) fn frame_of(seconds: f32) -> u32 {
    (seconds.max(0.0) * FRAMES_PER_SECOND).round() as u32
}

impl<'a> Plan<'a> {
    fn new(target: Target<'_>, clip: &'a Clip) -> Self {
        let model = &*clip.model;
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
        let expressed: Vec<&AnimatedExpression> = vrma
            .iter()
            .flat_map(|vrma| &vrma.expressions)
            .filter(|expression| moved.contains_key(&expression.node))
            .collect();
        let targets = node_names(target.model);
        let source_drives = |node: NodeId| {
            clip.body
                .as_ref()
                .is_some_and(|body| body.mapping().is_driven(node))
        };
        let details: Vec<Detail<'_>> = moved
            .keys()
            .filter(|node| !signals.contains(node) && !source_drives(**node))
            .filter_map(|node| {
                let name = model.node(*node).name.as_str();
                let target_node = targets.get(name)?;
                (!target.drives(*target_node)).then_some(Detail {
                    source: *node,
                    name,
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
        Self {
            clip,
            vrma,
            moved,
            details,
            weighted,
            expressed,
        }
    }

    fn keyed(&self, node: NodeId, frame: u32) -> bool {
        self.moved
            .get(&node)
            .is_some_and(|frames| frames.contains(&frame))
    }

    fn sample(&self, sampling: Sampling<'_>) -> Sampled {
        let model = &self.clip.model;
        let animation = self.clip.animation();
        let reading = match sampling {
            Sampling::Keys => None,
            Sampling::Poses { target } => self.clip.body.as_deref().map(|body| (body, target)),
        };
        let last = frame_of(animation.duration());
        let chunks = PARALLEL
            .grain(FRAMES_PER_WORKER)
            .map_chunks(last as usize + 1, |range| {
                let mut instance = Instance::new(Arc::clone(model));
                let mut sampled = Sampled::default();
                for frame in range {
                    let frame = frame as u32;
                    instance.pose_mut().reset(model);
                    animation.apply(frame as f32 / FRAMES_PER_SECOND, instance.pose_mut());
                    if let Some((body, target)) = reading {
                        instance.refresh();
                        sampled.poses.push(self.read(body, target, &instance));
                    }
                    self.keys(frame, instance.pose(), &mut sampled);
                }
                sampled
            });
        let mut sampled = Sampled::default();
        for chunk in chunks {
            sampled.poses.extend(chunk.poses);
            sampled.bones.extend(chunk.bones);
            sampled.weights.extend(chunk.weights);
        }
        sampled
    }

    fn read(&self, body: &Body, target: Option<&Body>, instance: &Instance) -> HumanPose {
        let mut pose = body.read(instance);
        if let (Some(gaze), Some(target)) = (
            self.vrma.and_then(|vrma| vrma.gaze(instance.pose())),
            target,
        ) {
            let turn = Quat::from_euler(EulerRot::YXZ, gaze.yaw, gaze.pitch, 0.0);
            for eye in [HumanBone::LeftEye, HumanBone::RightEye] {
                target.set_local_rotation(eye, turn, &mut pose);
            }
        }
        pose
    }

    fn keys(&self, frame: u32, pose: &Pose, sampled: &mut Sampled) {
        let model = &self.clip.model;
        let key = |morph: &str, weight: f32| MorphKey {
            morph: morph.to_owned(),
            frame,
            weight,
            curve: VmdCurve::LINEAR,
        };
        for detail in self
            .details
            .iter()
            .filter(|detail| self.keyed(detail.source, frame))
        {
            let rest = model.node(detail.source).transform;
            let local = pose.local(detail.source);
            sampled.bones.push(VmdBoneFrame {
                bone: detail.name.to_owned(),
                frame,
                translation: local.translation - rest.translation,
                rotation: (rest.rotation.inverse() * local.rotation).normalize(),
                interpolation: VmdInterpolation::default(),
                physics_off: false,
            });
        }
        for source in self
            .weighted
            .iter()
            .filter(|source| source.frames.contains(&frame))
        {
            for (name, weight) in source.names.iter().zip(pose.weights(source.node)) {
                sampled.weights.push(key(name, *weight));
            }
        }
        for expression in self
            .expressed
            .iter()
            .filter(|expression| self.keyed(expression.node, frame))
        {
            sampled
                .weights
                .push(key(expression.name.as_str(), expression.weight(pose)));
        }
    }
}

impl Take {
    #[must_use]
    pub fn from_clip(target: Target<'_>, clip: &Clip) -> Self {
        let plan = Plan::new(target, clip);
        let Sampled {
            poses,
            bones,
            weights,
        } = plan.sample(Sampling::Poses {
            target: target.body,
        });
        let keyed: Vec<(NodeId, u32)> = plan
            .moved
            .iter()
            .flat_map(|(node, frames)| frames.iter().map(|frame| (*node, *frame)))
            .collect();
        let human = clip
            .body
            .as_deref()
            .zip(target.body)
            .and_then(|(body, target_body)| {
                let source = Source {
                    model: &clip.model,
                    body,
                };
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

pub(crate) fn details(target: Target<'_>, clip: &Clip) -> Take {
    let Sampled { bones, weights, .. } = Plan::new(target, clip).sample(Sampling::Keys);
    Take {
        bones: bones.into_iter().collect(),
        morphs: morphs(target, weights),
        ..Take::default()
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
