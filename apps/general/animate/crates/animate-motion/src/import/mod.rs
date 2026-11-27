mod adopting;
mod clip;
mod contact;
mod reduce;
mod retarget;
mod seeds;
mod standard;
mod vmd;

use std::sync::Arc;

use animate_body::{Body, Faces, HumanPose, Limb, Side, Worlds};
use rikka_model_core::{Model, NodeId};
use rustc_hash::FxHashMap;

use crate::PARALLEL;
use crate::motion::HumanMotion;
use crate::part::{Goal, Part};
use crate::sample::{Pinned, PoseSample};

use contact::{Footing, footing};
use reduce::reduce;
use seeds::Seeds;

pub use clip::Clip;
pub use retarget::{Recipient, Retarget};

const FRAMES_PER_WORKER: usize = 32;

#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    pub model: &'a Arc<Model>,
    pub body: Option<&'a Body>,
    pub faces: &'a Faces,
}

impl Target<'_> {
    fn drives(&self, node: NodeId) -> bool {
        self.body.is_some_and(|body| body.mapping().is_driven(node))
    }
}

#[derive(Clone, Copy)]
struct Source<'a> {
    model: &'a Arc<Model>,
    body: &'a Body,
}

pub(crate) struct Performer {
    pub(crate) model: Arc<Model>,
    pub(crate) body: Arc<Body>,
}

impl Performer {
    fn source(&self) -> Source<'_> {
        Source {
            model: &self.model,
            body: &self.body,
        }
    }
}

pub(crate) fn node_names(model: &Model) -> FxHashMap<&str, NodeId> {
    model
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.name.as_str(), NodeId(index as u32)))
        .collect()
}

struct Recording {
    poses: Vec<HumanPose>,
    keyed: Vec<(NodeId, u32)>,
}

fn human_motion(source: Source<'_>, target: &Body, recording: &Recording) -> Option<HumanMotion> {
    let seeds = Seeds::new(source.model, source.body, &recording.keyed);
    let mut human = HumanMotion::default();
    add_body(&mut human, target, &recording.poses, &seeds);
    (!human.is_empty()).then_some(human)
}

fn add_body(human: &mut HumanMotion, body: &Body, poses: &[HumanPose], seeds: &Seeds) {
    let worlds: Vec<Worlds> = PARALLEL
        .grain(FRAMES_PER_WORKER)
        .collect(poses.len(), |frame| body.fk(&poses[frame]));
    let feet = Side::BOTH.map(|side| footing(body, &worlds, side));
    let footing_of = |limb: Limb| match limb {
        Limb::LeftLeg => Some(&feet[0]),
        Limb::RightLeg => Some(&feet[1]),
        Limb::LeftArm | Limb::RightArm => None,
    };
    let scale = body.scale();
    let pose_sample: Vec<PoseSample> =
        PARALLEL
            .grain(FRAMES_PER_WORKER)
            .collect(poses.len(), |frame| {
                let mut pose_sample = PoseSample::resting(&poses[frame]);
                for limb in Limb::ALL {
                    pose_sample.pins[limb.index()] = match footing_of(limb) {
                        Some(footing) => Pinned::new(footing.pinned[frame], footing.goals[frame]),
                        None => Pinned::new(false, Goal::from_worlds(&worlds[frame], limb, scale)),
                    };
                }
                pose_sample
            });
    let reduced = PARALLEL.map(Part::ALL, |part| {
        let values: Vec<_> = pose_sample
            .iter()
            .map(|sample| sample.values(part))
            .collect();
        let pinned: &[bool] = part
            .limb()
            .and_then(footing_of)
            .map_or(&[], |footing: &Footing| footing.pinned.as_slice());
        reduce(
            part,
            &values,
            &switches(pinned),
            pinned,
            seeds.keyed_frames(part),
        )
    });
    for key in reduced.into_iter().flatten() {
        human.insert(key);
    }
}

fn switches(pinned: &[bool]) -> Vec<u32> {
    pinned
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] != pair[1])
        .flat_map(|(at, _)| [at as u32, at as u32 + 1])
        .collect()
}
