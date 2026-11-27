use std::sync::Arc;

use animate_body::{Body, Faces, HumanBone, HumanPose, MorphWeights};
use rikka_mmd::{Puppet, Vmd, VmdBoneFrame, VmdSwitches};
use rikka_model_core::glam::{EulerRot, Quat, Vec3};
use rikka_model_core::{Instance, Model, NodeId};
use rikka_vrm::{Expressive, VrmAnimation};

use super::clip::{self, Clip};
use super::standard::STANDARD;
use super::vmd::{self as vmd_import, plays_natively};
use super::{Target, node_names};
use crate::detail_playback::DetailPlayback;
use crate::morph_playback::MorphPlayback;
use crate::sample::PoseSample;
use crate::take::Take;
use crate::tuning::FRAMES_PER_SECOND;

#[derive(Clone, Debug)]
pub struct Recipient {
    pub model: Arc<Model>,
    pub body: Option<Arc<Body>>,
    pub faces: Arc<Faces>,
}

impl Recipient {
    #[must_use]
    pub fn target(&self) -> Target<'_> {
        Target {
            model: &self.model,
            body: self.body.as_deref(),
            faces: &self.faces,
        }
    }
}

#[derive(Debug)]
pub struct Retarget {
    recipient: Recipient,
    take: Arc<Take>,
    detail: DetailPlayback,
    morphs: MorphPlayback,
    rig: Option<Rig>,
    frames: u32,
}

#[derive(Debug)]
struct Rig {
    body: Arc<Body>,
    instance: Instance,
    motion: Played,
}

#[derive(Debug)]
enum Played {
    Keys {
        bones: Vec<Keyed>,
        switches: VmdSwitches,
    },
    Clip {
        animation: usize,
    },
}

#[derive(Debug)]
struct Keyed {
    node: NodeId,
    rest: Vec3,
    keys: Vec<VmdBoneFrame>,
}

impl Keyed {
    fn at(&self, frame: f32) -> (Vec3, Quat) {
        let next = self.keys.partition_point(|key| key.frame as f32 <= frame);
        let from = &self.keys[next.saturating_sub(1)];
        match (next, self.keys.get(next)) {
            (0, _) | (_, None) => (from.translation, from.rotation),
            (_, Some(to)) => {
                let factor = (frame - from.frame as f32) / (to.frame - from.frame) as f32;
                from.toward(to, factor)
            }
        }
    }
}

impl Rig {
    fn keyed(recipient: &Recipient, vmd: &Vmd) -> Option<Self> {
        let (model, body) = match plays_natively(recipient.target()) {
            true => (
                Arc::clone(&recipient.model),
                Arc::clone(recipient.body.as_ref()?),
            ),
            false => (Arc::clone(&STANDARD.model), Arc::clone(&STANDARD.body)),
        };
        let nodes = node_names(&model);
        let bones: Vec<Keyed> = vmd
            .bones_by_name()
            .into_iter()
            .filter_map(|(name, frames)| {
                let node = *nodes.get(name)?;
                Some(Keyed {
                    node,
                    rest: model.node(node).transform.translation,
                    keys: frames.into_iter().cloned().collect(),
                })
            })
            .collect();
        let mapping = body.mapping();
        if !bones.iter().any(|keyed| mapping.is_driven(keyed.node)) {
            return None;
        }
        Some(Self {
            motion: Played::Keys {
                bones,
                switches: vmd.switches(&model),
            },
            instance: Instance::new(model),
            body,
        })
    }

    fn read(&mut self, frame: f32, target: &Body) -> HumanPose {
        let model = Arc::clone(self.instance.model());
        let seconds = frame / FRAMES_PER_SECOND;
        self.instance.pose_mut().reset(&model);
        match &self.motion {
            Played::Keys { bones, switches } => {
                let pose = self.instance.pose_mut();
                for keyed in bones {
                    let (translation, rotation) = keyed.at(frame);
                    if let Some(local) = pose.local_mut(keyed.node) {
                        local.translation = keyed.rest + translation;
                        local.rotation = rotation;
                    }
                }
                if let Some(puppet) = self.instance.rig_as_mut::<Puppet>() {
                    switches.apply(seconds, puppet);
                }
            }
            Played::Clip { animation } => {
                model.animations[*animation].apply(seconds, self.instance.pose_mut());
            }
        }
        self.instance.refresh();
        let mut pose = self.body.read(&self.instance);
        let gaze = model
            .extensions
            .get::<VrmAnimation>()
            .and_then(|vrma| vrma.gaze(self.instance.pose()));
        if let Some(gaze) = gaze {
            let turn = Quat::from_euler(EulerRot::YXZ, gaze.yaw, gaze.pitch, 0.0);
            for eye in [HumanBone::LeftEye, HumanBone::RightEye] {
                target.set_local_rotation(eye, turn, &mut pose);
            }
        }
        pose
    }
}

impl Retarget {
    #[must_use]
    pub fn from_take(recipient: Recipient, take: Arc<Take>) -> Self {
        let frames = take.last_frame() + 1;
        Self::with(recipient, take, None, frames)
    }

    #[must_use]
    pub fn from_vmd(recipient: Recipient, vmd: &Vmd) -> Self {
        let take = vmd_import::details(recipient.target(), vmd);
        let rig = recipient
            .body
            .as_ref()
            .and_then(|_| Rig::keyed(&recipient, vmd));
        Self::with(recipient, Arc::new(take), rig, vmd.last_frame() + 1)
    }

    #[must_use]
    pub fn from_clip(recipient: Recipient, clip: &Clip) -> Self {
        let take = clip::details(recipient.target(), clip);
        let rig = recipient.body.as_ref().and(clip.body()).map(|body| Rig {
            body: Arc::clone(body),
            instance: Instance::new(Arc::clone(clip.model())),
            motion: Played::Clip {
                animation: clip.index(),
            },
        });
        let frames = clip::frame_of(clip.animation().duration()) + 1;
        Self::with(recipient, Arc::new(take), rig, frames)
    }

    fn with(recipient: Recipient, take: Arc<Take>, rig: Option<Rig>, frames: u32) -> Self {
        Self {
            detail: DetailPlayback::new(&recipient.model, Arc::clone(&take)),
            morphs: MorphPlayback::new(Arc::clone(&recipient.faces), Arc::clone(&take)),
            recipient,
            take,
            rig,
            frames,
        }
    }

    #[must_use]
    pub fn recipient(&self) -> &Recipient {
        &self.recipient
    }

    #[must_use]
    pub fn frames(&self) -> u32 {
        self.frames
    }

    pub fn pose_at(&mut self, frame: f32, eye: Vec3) -> Option<HumanPose> {
        let body = self.recipient.body.as_deref()?;
        let pose_sample = match &mut self.rig {
            Some(rig) => PoseSample::resting(&rig.read(frame, body)),
            None if self.take.human.is_some() => self.take.sample(frame, body.rest_pose()),
            None => return None,
        };
        Some(pose_sample.reached(body, || eye))
    }

    pub fn write(&mut self, seconds: f32, eye: Vec3, instance: &mut Instance) -> Option<HumanPose> {
        let model = Arc::clone(instance.model());
        instance.pose_mut().reset(&model);
        instance.clear_expressions();
        self.detail.write(seconds, instance.pose_mut());
        if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
            self.detail.switch(seconds, puppet);
        }
        let human = self.pose_at(seconds * FRAMES_PER_SECOND, eye);
        if let (Some(body), Some(pose)) = (self.recipient.body.as_deref(), human.as_ref()) {
            body.write(pose, instance.pose_mut());
            if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
                body.release(puppet);
            }
        }
        self.morphs
            .write(seconds, &MorphWeights::default(), instance);
        human
    }
}
