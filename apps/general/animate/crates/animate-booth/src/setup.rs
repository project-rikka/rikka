use std::sync::Arc;

use animate_body::{Body, HumanPose};
use animate_cast::Shot;
use animate_motion::{CameraMotion, Retarget};
use rikka_model::glam::Vec3;
use rikka_model::humanoid::Anatomy;
use rikka_model::{Bounds, HumanBone, Instance, Model, NodeId};
use rikka_model_gpu::HeadRest;
use rikka_sync::Mutex;

use crate::flesh::Flesh;
use crate::framing::{Aim, fit};
use crate::sitter::{ModelKey, Source};

#[derive(Debug)]
pub(crate) struct Facts {
    pub(crate) head: Option<HeadRest>,
    hips: Option<(NodeId, Vec3)>,
    flesh: Flesh,
    pub(crate) forward: Vec3,
    pub(crate) humanoid: bool,
}

impl Facts {
    pub(crate) fn new(model: &Model) -> Self {
        let anatomy = Anatomy::recognize(model);
        let rest = model.world_transforms();
        let humanoid = anatomy.humanoid();
        Self {
            head: HeadRest::new(model, &anatomy),
            hips: humanoid
                .bone(HumanBone::Hips)
                .map(|hips| (hips, rest[hips.index()].w_axis.truncate())),
            flesh: Flesh::new(model),
            forward: anatomy.forward(),
            humanoid: !humanoid.is_empty() && humanoid.missing().next().is_none(),
        }
    }

    fn track(&self, instance: &Instance) -> Vec3 {
        self.hips.map_or(Vec3::ZERO, |(hips, rest)| {
            (instance.world(hips).w_axis.truncate() - rest).with_y(0.0)
        })
    }

    fn posed_bounds(&self, instance: &Instance) -> Bounds {
        self.flesh
            .bounds(instance)
            .unwrap_or_else(|| instance.bounds())
    }
}

#[derive(Debug)]
pub(crate) enum Posture {
    Rest,
    Standing(Box<HumanPose>),
    Retarget(Box<Mutex<Retarget>>),
}

#[derive(Debug)]
pub(crate) struct Performance {
    pub(crate) posture: Posture,
    pub(crate) body: Option<Arc<Body>>,
    pub(crate) facts: Arc<Facts>,
}

impl Performance {
    pub(crate) fn pose(&self, instance: &mut Instance, time: f32) {
        let body = self.body.as_deref();
        match &self.posture {
            Posture::Rest => animate_stage::rest(instance, body),
            Posture::Standing(standing) => {
                animate_stage::rest(instance, body);
                if let Some(body) = body {
                    body.write(standing, instance.pose_mut());
                    instance.refresh();
                }
            }
            Posture::Retarget(retarget) => {
                retarget.lock().write(time, Shot::default().eye(), instance);
                instance.refresh();
            }
        }
    }

    pub(crate) fn extent(
        &self,
        instance: &mut Instance,
        times: impl IntoIterator<Item = f32>,
    ) -> Bounds {
        let mut extent: Option<Bounds> = None;
        for time in times {
            self.pose(instance, time);
            let track = self.facts.track(instance);
            let bounds = match self.posture {
                Posture::Rest => instance.bounds(),
                Posture::Standing(_) | Posture::Retarget(_) => self.facts.posed_bounds(instance),
            };
            let relative = Bounds {
                min: bounds.min - track,
                max: bounds.max - track,
            };
            extent = Some(extent.map_or(relative, |known| known.union(relative)));
        }
        extent.unwrap_or_else(|| instance.bounds())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Still,
    Loop,
}

#[derive(Clone, Debug)]
pub(crate) enum View {
    Framed {
        still: Bounds,
        extent: Bounds,
        aim: Aim,
    },
    Camera(Arc<CameraMotion>),
}

#[derive(Debug)]
pub(crate) struct Setup {
    pub(crate) key: ModelKey,
    pub(crate) source: Source,
    pub(crate) performance: Performance,
    pub(crate) view: View,
    pub(crate) still: f32,
    pub(crate) length: f32,
}

impl Setup {
    pub(crate) fn head(&self) -> Option<&HeadRest> {
        self.performance.facts.head.as_ref()
    }

    pub(crate) fn pose(&self, instance: &mut Instance, time: f32) {
        self.performance.pose(instance, time);
    }

    pub(crate) fn shot(&self, instance: &Instance, time: f32, aspect: f32, scope: Scope) -> Shot {
        match &self.view {
            View::Framed { still, extent, aim } => {
                let framed = match scope {
                    Scope::Still => still,
                    Scope::Loop => extent,
                };
                let track = self.performance.facts.track(instance);
                let bounds = Bounds {
                    min: framed.min + track,
                    max: framed.max + track,
                };
                fit(bounds, *aim, aspect)
            }
            View::Camera(motion) => motion.pose_at(time).map_or_else(Shot::default, Shot::from),
        }
    }
}
