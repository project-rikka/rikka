use std::sync::Arc;

use animate_body::{Body, HumanPose};
use rikka_mmd::{Pmx, Puppet, Vmd, VmdCurve, VmdPropertyFrame};
use rikka_model_core::{Instance, NodeId};

use super::adopting::Adopting;
use super::standard::STANDARD;
use super::{FRAMES_PER_WORKER, Recording, Source, Target, human_motion, node_names};
use crate::PARALLEL;
use crate::morphs::{MorphKey, Morphs};
use crate::motion::HumanMotion;
use crate::take::Take;
use crate::tuning::FRAMES_PER_SECOND;

impl Take {
    #[must_use]
    pub fn from_vmd(target: Target<'_>, vmd: Vmd) -> Self {
        let human = target
            .body
            .zip(source_of(target))
            .and_then(|(body, source)| human(source, body, &vmd));
        Take {
            human,
            ..details(target, &vmd)
        }
    }
}

pub(crate) fn plays_natively(target: Target<'_>) -> bool {
    target.model.extensions.contains::<Pmx>()
}

fn source_of(target: Target<'_>) -> Option<Source<'_>> {
    match plays_natively(target) {
        true => target.body.map(|body| Source {
            model: target.model,
            body,
        }),
        false => Some(STANDARD.source()),
    }
}

pub(crate) fn details(target: Target<'_>, vmd: &Vmd) -> Take {
    let morphs = morphs(target, vmd);
    if !plays_natively(target) {
        return Take {
            morphs,
            ..Take::default()
        };
    }
    let nodes = node_names(target.model);
    let driven = |name: &str| nodes.get(name).is_some_and(|node| target.drives(*node));
    Take {
        human: None,
        bones: vmd
            .bones
            .iter()
            .filter(|frame| !driven(&frame.bone))
            .cloned()
            .collect(),
        switches: vmd
            .properties
            .iter()
            .map(|frame| VmdPropertyFrame {
                ik: frame
                    .ik
                    .iter()
                    .filter(|(name, _)| !driven(name))
                    .cloned()
                    .collect(),
                ..frame.clone()
            })
            .collect(),
        morphs,
        holds: animate_hold::Holds::default(),
    }
}

fn human(source: Source<'_>, target: &Body, vmd: &Vmd) -> Option<HumanMotion> {
    let nodes = node_names(source.model);
    let keyed: Vec<(NodeId, u32)> = vmd
        .bones
        .iter()
        .filter_map(|frame| Some((*nodes.get(frame.bone.as_str())?, frame.frame)))
        .collect();
    let mapping = source.body.mapping();
    if !keyed.iter().any(|(node, _)| mapping.is_driven(*node)) {
        return None;
    }
    let recording = Recording {
        poses: played(source, vmd),
        keyed,
    };
    human_motion(source, target, &recording)
}

fn morphs(target: Target<'_>, vmd: &Vmd) -> Morphs {
    let mut adopting = Adopting::new(target.faces);
    let mut morphs = Morphs::default();
    for frame in &vmd.morphs {
        let key = MorphKey {
            morph: frame.morph.clone(),
            frame: frame.frame,
            weight: frame.weight,
            curve: VmdCurve::LINEAR,
        };
        for adopted in adopting.keys(key) {
            morphs.insert(adopted);
        }
    }
    morphs
}

fn played(source: Source<'_>, vmd: &Vmd) -> Vec<HumanPose> {
    let model = source.model;
    let animation = vmd.animation(model);
    let switches = vmd.switches(model);
    let frames = vmd.last_frame() as usize + 1;
    PARALLEL
        .grain(FRAMES_PER_WORKER)
        .concat_chunks(frames, |range| {
            let mut instance = Instance::new(Arc::clone(model));
            range
                .map(|frame| {
                    let time = frame as f32 / FRAMES_PER_SECOND;
                    instance.pose_mut().reset(model);
                    animation.apply(time, instance.pose_mut());
                    if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
                        switches.apply(time, puppet);
                    }
                    instance.refresh();
                    source.body.read(&instance)
                })
                .collect()
        })
}
