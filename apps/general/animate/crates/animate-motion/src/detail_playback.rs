use std::sync::Arc;

use rikka_mmd::{Pmx, Puppet};
use rikka_model_core::{Model, NodeId, Pose, Transform};
use rustc_hash::FxHashMap;

use crate::import::node_names;
use crate::take::Take;
use crate::tuning::FRAMES_PER_SECOND;

const SETTLED: f32 = 1e-3;

#[derive(Clone, Copy, Debug)]
struct Placed {
    node: NodeId,
    rest: Transform,
}

#[derive(Clone, Debug)]
pub struct DetailPlayback {
    take: Arc<Take>,
    placed: Arc<[Option<Placed>]>,
    solvers: Arc<FxHashMap<String, usize>>,
}

impl DetailPlayback {
    #[must_use]
    pub fn new(model: &Model, take: Arc<Take>) -> Self {
        let mut solvers = FxHashMap::default();
        if let Some(pmx) = model.extensions.get::<Pmx>() {
            for (index, bone) in pmx.bones.iter().enumerate() {
                if bone.ik.is_some() {
                    solvers.entry(bone.name.clone()).or_insert(index);
                }
            }
        }
        Self {
            placed: place(model, &take),
            take,
            solvers: Arc::new(solvers),
        }
    }

    #[must_use]
    pub fn rebound(&self, model: &Model, take: Arc<Take>) -> Self {
        let placed = match self.take.bones.keyed().eq(take.bones.keyed()) {
            true => Arc::clone(&self.placed),
            false => place(model, &take),
        };
        Self {
            take,
            placed,
            solvers: Arc::clone(&self.solvers),
        }
    }

    pub fn write(&self, seconds: f32, pose: &mut Pose) {
        let frame = seconds * FRAMES_PER_SECOND;
        for (bone, placed) in self.take.bones.keyed().zip(self.placed.iter()) {
            let Some(placed) = placed else {
                continue;
            };
            let Some((translation, rotation)) = self.take.bones.value_at(bone, frame) else {
                continue;
            };
            if let Some(local) = pose.local_mut(placed.node) {
                local.translation = placed.rest.translation + translation;
                local.rotation = (placed.rest.rotation * rotation).normalize();
            }
        }
    }

    pub fn switch(&self, seconds: f32, puppet: &mut Puppet) {
        puppet.reach_everywhere();
        let Some(switched) = self.take.switches.held_at(&(), switch_frame(seconds)) else {
            return;
        };
        for (name, reaching) in &switched.ik {
            if let Some(bone) = self.solvers.get(name) {
                puppet.set_reaching(*bone, *reaching);
            }
        }
    }

    #[must_use]
    pub fn visible(&self, seconds: f32) -> bool {
        self.take
            .switches
            .held_at(&(), switch_frame(seconds))
            .is_none_or(|switched| switched.visible)
    }
}

fn switch_frame(seconds: f32) -> u32 {
    (seconds * FRAMES_PER_SECOND + SETTLED).floor().max(0.0) as u32
}

fn place(model: &Model, take: &Take) -> Arc<[Option<Placed>]> {
    let nodes = node_names(model);
    take.bones
        .keyed()
        .map(|bone| {
            let node = *nodes.get(bone.as_str())?;
            Some(Placed {
                node,
                rest: model.node(node).transform,
            })
        })
        .collect()
}
