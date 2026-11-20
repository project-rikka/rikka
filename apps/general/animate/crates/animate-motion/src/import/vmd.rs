use std::sync::Arc;

use animate_body::{Body, HumanPose};
use rikka_mmd::{Pmx, Puppet, Vmd, VmdCurve, VmdPropertyFrame};
use rikka_model_core::{Instance, NodeId};
use rustc_hash::FxHashMap;

use super::adopting::Adopting;
use super::standard::STANDARD;
use super::{Recording, Source, Target, human_motion, node_names};
use crate::morphs::{MorphKey, Morphs};
use crate::motion::HumanMotion;
use crate::take::Take;
use crate::tuning::FRAMES_PER_SECOND;

impl Take {
    #[must_use]
    pub fn from_vmd(target: Target<'_>, vmd: Vmd) -> Self {
        match target.model.extensions.contains::<Pmx>() {
            true => native(target, &vmd),
            false => standard(target, &vmd),
        }
    }
}

fn native(target: Target<'_>, vmd: &Vmd) -> Take {
    let nodes = node_names(target.model);
    let driven = |name: &str| nodes.get(name).is_some_and(|node| target.drives(*node));
    let human = target.body.and_then(|body| {
        let source = Source {
            model: target.model,
            body,
        };
        human(source, &nodes, body, vmd)
    });
    Take {
        human,
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
        morphs: morphs(target, vmd),
        holds: animate_hold::Holds::default(),
    }
}

fn standard(target: Target<'_>, vmd: &Vmd) -> Take {
    let performer = &*STANDARD;
    let human = target.body.and_then(|body| {
        let nodes = node_names(&performer.model);
        human(performer.source(), &nodes, body, vmd)
    });
    Take {
        human,
        morphs: morphs(target, vmd),
        ..Take::default()
    }
}

fn human(
    source: Source<'_>,
    nodes: &FxHashMap<&str, NodeId>,
    target: &Body,
    vmd: &Vmd,
) -> Option<HumanMotion> {
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
    let mut instance = Instance::new(Arc::clone(model));
    let animation = vmd.animation(model);
    let switches = vmd.switches(model);
    (0..=vmd.last_frame())
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
}
