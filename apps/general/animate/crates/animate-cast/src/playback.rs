use std::sync::Arc;

use animate_body::{Body, Faces};
use animate_fit::Physique;
use animate_hold::Holds;
use animate_motion::{CameraMotion, DetailPlayback, MorphPlayback, MotionBake, Take};
use rikka_model::Model;
use rikka_model::glam::Vec3;
use rikka_model::mmd::FRAMES_PER_SECOND;

use crate::shot::Shot;

#[derive(Clone, Debug, Default)]
pub struct Eye {
    camera: Option<Arc<CameraMotion>>,
}

impl Eye {
    pub fn new(camera: Option<Arc<CameraMotion>>) -> Self {
        Self { camera }
    }

    pub fn at(&self, frame: u32) -> Vec3 {
        self.camera
            .as_ref()
            .and_then(|camera| camera.pose_at(frame as f32 / FRAMES_PER_SECOND))
            .map_or_else(|| Shot::default().eye(), |pose| pose.eye())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Performer<'a> {
    pub body: Option<&'a Arc<Body>>,
    pub physique: Option<&'a Arc<Physique>>,
    pub faces: &'a Arc<Faces>,
}

#[derive(Debug)]
pub struct TakePlayback {
    detail: DetailPlayback,
    motion_bake: Option<Arc<MotionBake>>,
    morph_playback: MorphPlayback,
    holds: Arc<Holds>,
}

impl TakePlayback {
    pub fn bind(model: &Model, take: &Arc<Take>, performer: Performer<'_>, eye: &Eye) -> Self {
        Self {
            detail: DetailPlayback::new(model, Arc::clone(take)),
            motion_bake: bake(take, performer, eye),
            morph_playback: morph_playback(take, performer),
            holds: Arc::new(take.holds.clone()),
        }
    }

    pub fn rebind(
        &self,
        model: &Model,
        before: &Take,
        take: &Arc<Take>,
        performer: Performer<'_>,
        eye: &Eye,
    ) -> Self {
        let detail = self.detail.rebound(model, Arc::clone(take));
        let motion_bake = match (&self.motion_bake, &before.human, &take.human) {
            (Some(motion_bake), Some(was), Some(human)) => match human.dirty_since(was) {
                Some(dirty) => Some(Arc::new(
                    motion_bake.rebaked(human, dirty, |frame| eye.at(frame)),
                )),
                None => Some(Arc::clone(motion_bake)),
            },
            _ => bake(take, performer, eye),
        };
        let holds = match before.holds == take.holds {
            true => Arc::clone(&self.holds),
            false => Arc::new(take.holds.clone()),
        };
        Self {
            detail,
            motion_bake,
            morph_playback: morph_playback(take, performer),
            holds,
        }
    }

    pub fn rebaked(&self, take: &Arc<Take>, performer: Performer<'_>, eye: &Eye) -> Self {
        Self {
            detail: self.detail.clone(),
            motion_bake: bake(take, performer, eye),
            morph_playback: morph_playback(take, performer),
            holds: Arc::clone(&self.holds),
        }
    }

    pub fn detail(&self) -> &DetailPlayback {
        &self.detail
    }

    pub fn motion_bake(&self) -> Option<&Arc<MotionBake>> {
        self.motion_bake.as_ref()
    }

    pub fn morph_playback(&self) -> &MorphPlayback {
        &self.morph_playback
    }

    pub fn holds(&self) -> &Arc<Holds> {
        &self.holds
    }
}

fn morph_playback(take: &Arc<Take>, performer: Performer<'_>) -> MorphPlayback {
    MorphPlayback::new(Arc::clone(performer.faces), Arc::clone(take))
}

fn bake(take: &Take, performer: Performer<'_>, eye: &Eye) -> Option<Arc<MotionBake>> {
    let (body, human) = performer.body.zip(take.human.as_ref())?;
    Some(Arc::new(MotionBake::new(
        Arc::clone(body),
        performer.physique.cloned(),
        human,
        |frame| eye.at(frame),
    )))
}
