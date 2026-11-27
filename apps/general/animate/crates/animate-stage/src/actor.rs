use std::sync::Arc;

use animate_body::{Body, Faces, HandPreset, HumanBone, HumanPose, MorphWeights, Side, Worlds};
use animate_cast::{FigureId, Skeleton, TakePlayback};
use animate_hold::Holds;
use animate_look::Name;
use animate_motion::{MotionBake, Written};
use rikka_model::glam::{EulerRot, Mat4, Quat, Vec3};
use rikka_model::mmd::{FRAMES_PER_SECOND, Puppet};
use rikka_model::vrm::{Avatar, Expressive, Gaze, LookAtKind, Vrm};
use rikka_model::{Instance, NodeId, Transform};
use rikka_model_gpu::HeadRest;
use rikka_physics::FigureMotion;

use crate::holding::{GripPoint, Pinches};
use crate::pending::{Pending, PendingPose};
use crate::pour::CastMember;

#[derive(Clone, Debug, PartialEq)]
pub struct Projection {
    pub figure: FigureId,
    pub view_projection: Mat4,
    pub worlds: Vec<Mat4>,
    pub human: Option<Worlds>,
    pub grips: Vec<GripPoint>,
}

impl Projection {
    pub fn turn_of(&self, bone: Option<usize>) -> Quat {
        bone.and_then(|bone| self.worlds.get(bone))
            .map_or(Quat::IDENTITY, |world| {
                let (_, rotation, _) = world.to_scale_rotation_translation();
                rotation
            })
    }
}

#[derive(Debug, Default, PartialEq)]
enum Holding {
    #[default]
    Nothing,
    Bones(Vec<(NodeId, Transform)>),
    Human(Box<Previewed>),
}

#[derive(Debug, PartialEq)]
struct Previewed {
    pose: HumanPose,
    locals: Vec<Transform>,
}

pub(super) struct Actor {
    pub(super) id: FigureId,
    pub(super) name: String,
    pub(super) look_name: Name,
    pub(super) instance: Instance,
    pub(super) head: Option<HeadRest>,
    pub(super) skeleton: Arc<Skeleton>,
    pub(super) body: Option<Arc<Body>>,
    faces: Arc<Faces>,
    pub(super) pinches: Pinches,
    pub(super) playback: Option<Arc<TakePlayback>>,
    pub(super) visible: bool,
    pub(super) rested: bool,
    holding: Holding,
    morphs: MorphWeights,
    human: Option<HumanPose>,
    estimated: bool,
}

impl Actor {
    pub(super) fn cast(member: CastMember, instance: Instance) -> Self {
        Self {
            id: member.id,
            look_name: Name::new(&member.name),
            name: member.name,
            instance,
            head: member.head,
            skeleton: member.skeleton,
            body: member.body,
            faces: member.faces,
            pinches: member.pinches,
            playback: member.playback,
            visible: true,
            rested: false,
            holding: Holding::Nothing,
            morphs: MorphWeights::default(),
            human: None,
            estimated: false,
        }
    }

    pub(super) fn holds(&self) -> Option<&Holds> {
        self.playback
            .as_ref()
            .map(|playback| playback.holds().as_ref())
    }

    pub(super) fn baking(&self) -> Option<(&MotionBake, bool)> {
        let motion_bake = self.playback.as_ref()?.motion_bake()?;
        (!motion_bake.is_complete()).then_some((motion_bake.as_ref(), self.estimated))
    }

    pub(super) fn is_estimated(&self) -> bool {
        self.estimated
    }

    pub(super) fn animate(&mut self, time: f32) -> bool {
        if self.playback.is_none() && self.rested {
            return false;
        }
        let performance = perform(
            self.playback.as_deref(),
            self.body.as_deref(),
            &self.morphs,
            &mut self.instance,
            time,
        );
        self.human = performance.human;
        self.visible = performance.visible;
        self.estimated = performance.estimated;
        if performance.motion == FigureMotion::Resting {
            self.rested = self.holding == Holding::Nothing && self.morphs.is_empty();
        }
        match &self.holding {
            Holding::Nothing => {}
            Holding::Bones(bones) => {
                for (node, local) in bones {
                    self.instance.pose_mut().set_local(*node, *local);
                }
            }
            Holding::Human(previewed) => {
                if let Some(body) = &self.body {
                    write(&mut self.instance, body, &previewed.locals);
                    let pose = match &self.playback {
                        Some(playback) => close_hands(
                            playback.holds(),
                            body,
                            &mut self.instance,
                            time,
                            previewed.pose,
                        ),
                        None => previewed.pose,
                    };
                    self.human = Some(pose);
                }
            }
        }
        if self.playback.is_none() && !self.morphs.is_empty() {
            self.faces.write(&self.morphs, &mut self.instance);
        }
        if let (Some(body), Some(pose)) = (&self.body, &self.human) {
            look(&mut self.instance, body, pose);
        }
        true
    }

    pub(super) fn hold(&mut self, pending: Option<&Pending>) -> bool {
        let pending = pending.filter(|pending| pending.figure == self.id);
        let holding = match pending.and_then(|pending| pending.pose.as_ref()) {
            Some(PendingPose::Bones(bones)) => Holding::Bones(
                bones
                    .iter()
                    .filter_map(|(index, local)| {
                        let bone = self.skeleton.bone(*index)?;
                        Some((
                            bone.node,
                            Transform {
                                translation: bone.rest + local.translation,
                                rotation: bone.rest_rotation * local.rotation,
                                scale: Vec3::ONE,
                            },
                        ))
                    })
                    .collect(),
            ),
            Some(PendingPose::Human(posture)) => match &self.body {
                Some(body) => Holding::Human(Box::new(Previewed {
                    pose: posture.shown,
                    locals: body.locals(&posture.shown),
                })),
                None => Holding::Nothing,
            },
            None => Holding::Nothing,
        };
        let morphs = pending.map_or_else(MorphWeights::default, |pending| pending.morphs.clone());
        if holding == self.holding && morphs == self.morphs {
            return false;
        }
        self.holding = holding;
        self.morphs = morphs;
        self.rested = false;
        true
    }

    pub(super) fn bone_world(&self, bone: &str) -> Option<Mat4> {
        self.skeleton.world_of(&self.instance, bone)
    }

    pub(super) fn projection(&self, view_projection: Mat4) -> Projection {
        Projection {
            figure: self.id,
            view_projection,
            worlds: self
                .skeleton
                .bones()
                .iter()
                .map(|bone| self.instance.world(bone.node))
                .collect(),
            human: self
                .body
                .as_ref()
                .zip(self.human)
                .map(|(body, pose)| body.fk(&pose)),
            grips: Vec::new(),
        }
    }
}

pub(crate) struct Performance {
    pub motion: FigureMotion,
    visible: bool,
    human: Option<HumanPose>,
    estimated: bool,
}

pub(crate) fn perform(
    playback: Option<&TakePlayback>,
    body: Option<&Body>,
    morphs: &MorphWeights,
    instance: &mut Instance,
    time: f32,
) -> Performance {
    let model = Arc::clone(instance.model());
    instance.pose_mut().reset(&model);
    instance.clear_expressions();
    let mut human = body.map(|body| *body.rest_pose());
    let Some(playback) = playback else {
        if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
            puppet.reach_everywhere();
        }
        return Performance {
            motion: FigureMotion::Resting,
            visible: true,
            human,
            estimated: false,
        };
    };
    playback.detail().write(time, instance.pose_mut());
    if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
        playback.detail().switch(time, puppet);
    }
    let mut estimated = false;
    if let (Some(motion_bake), Some(body)) = (playback.motion_bake(), body) {
        let written = motion_bake.write(time, instance.pose_mut());
        release(instance, body);
        estimated = written == Written::Estimated;
        human = Some(match written {
            Written::Baked => fingers_between(motion_bake, time),
            Written::Estimated => motion_bake.pose_at(frame_at(time).round() as u32),
        });
    }
    playback.morph_playback().write(time, morphs, instance);
    if let (Some(body), Some(pose)) = (body, human) {
        human = Some(close_hands(playback.holds(), body, instance, time, pose));
    }
    Performance {
        motion: FigureMotion::Moving,
        visible: playback.detail().visible(time),
        human,
        estimated,
    }
}

fn frame_at(time: f32) -> f32 {
    (time * FRAMES_PER_SECOND).max(0.0)
}

fn fingers_between(motion_bake: &MotionBake, time: f32) -> HumanPose {
    let frame = frame_at(time);
    let mut pose = motion_bake.pose_at(frame.round() as u32);
    let before = motion_bake.pose_at(frame.floor() as u32);
    let after = motion_bake.pose_at(frame.floor() as u32 + 1);
    let share = frame.fract();
    for side in Side::BOTH {
        for muscle in side.fingers() {
            pose.muscles[muscle] =
                before.muscles[muscle] + (after.muscles[muscle] - before.muscles[muscle]) * share;
        }
    }
    pose
}

fn close_hands(
    holds: &Holds,
    body: &Body,
    instance: &mut Instance,
    time: f32,
    mut pose: HumanPose,
) -> HumanPose {
    let frame = time * FRAMES_PER_SECOND;
    let closing: Vec<Side> = Side::BOTH
        .into_iter()
        .filter(|side| {
            let closing = holds.closing(*side, frame);
            if closing > 0.0 {
                HandPreset::Pinch.blend(*side, &mut pose, closing);
            }
            closing > 0.0
        })
        .collect();
    body.write_fingers(&pose, &closing, instance.pose_mut());
    pose
}

fn write(instance: &mut Instance, body: &Body, locals: &[Transform]) {
    let pose = instance.pose_mut();
    for (node, local) in body.driven_nodes().iter().zip(locals) {
        pose.set_local(*node, *local);
    }
    release(instance, body);
}

fn release(instance: &mut Instance, body: &Body) {
    if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
        body.release(puppet);
    }
}

fn look(instance: &mut Instance, body: &Body, pose: &HumanPose) {
    let expressed = instance
        .model()
        .extensions
        .get::<Vrm>()
        .is_some_and(|vrm| vrm.look_at.kind == LookAtKind::Expression);
    let Some(avatar) = instance.rig_as_mut::<Avatar>() else {
        return;
    };
    let gaze = expressed.then(|| {
        let left = body.local_rotation(HumanBone::LeftEye, pose);
        let right = body.local_rotation(HumanBone::RightEye, pose);
        let (yaw, pitch, _) = left.slerp(right, 0.5).to_euler(EulerRot::YXZ);
        Gaze { yaw, pitch }
    });
    avatar.set_gaze(gaze);
}
