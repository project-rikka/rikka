use animate_inspector::HoldEditing;
use animate_inspector::MorphEditing;
use animate_inspector::PoseEditing;
use std::sync::Arc;

use animate_cast::{CameraTrack, FigureId, Shot, Subject};
use animate_edit::{Change, Channel, Clip, Key, KeyId, KeyKind, Keyed};
use animate_inspector::CurveEditing;
use animate_motion::{CameraMotion, Take};
use animate_stage::{Blowing, Local};
use animate_viewport::{Aim, ClothAim, FigureAim};
use rikka_model::mmd::{VmdBoneFrame, VmdCurve};
use rustc_hash::FxHashSet;

use crate::animate::Animate;
use crate::animate::edit::Edit;
use crate::animate::subject_motion::SubjectMotion;

impl Animate {
    pub(crate) fn aim(&self) -> Aim {
        match self.subject {
            Some(Subject::Figure(id)) if self.paints_cloth() => Aim::Cloth(ClothAim {
                id,
                brush: self.controls.brush,
                piece: self.controls.piece,
            }),
            Some(Subject::Figure(id)) => self.cast.figure(id).map_or(Aim::Idle, |figure| {
                Aim::Figure(FigureAim {
                    id,
                    skeleton: Arc::clone(&figure.skeleton),
                    body: self.is_easy_mode().then(|| figure.body.clone()).flatten(),
                    faces: Arc::clone(&figure.faces),
                    take: Arc::clone(&figure.take),
                    playback: figure.playback.clone(),
                    paths: self.controls.paths,
                })
            }),
            Some(Subject::Emitter(id)) => Aim::Emitter(id),
            Some(Subject::Camera) => Aim::Camera,
            Some(Subject::Wind) => Aim::Wind,
            Some(Subject::Look) | None => Aim::Idle,
        }
    }

    pub(crate) fn take_of(&self, id: FigureId) -> Arc<Take> {
        self.cast
            .figure(id)
            .map_or_else(Arc::default, |figure| Arc::clone(&figure.take))
    }

    pub(crate) fn camera_motion(&self) -> Option<Arc<CameraMotion>> {
        self.cast.camera().map(|camera| Arc::clone(&camera.motion))
    }

    fn subject_motion(&self) -> Option<(Subject, SubjectMotion)> {
        let subject = self.subject?;
        Some((subject, self.motion_of(subject)?))
    }

    pub(super) fn record(&mut self, subject: Subject, edit: Edit) {
        self.history.record_in(self.undo_group, subject, edit);
    }

    pub(super) fn in_undo_group(&mut self, work: impl FnOnce(&mut Self)) {
        if self.undo_group.is_some() {
            work(self);
            return;
        }
        self.undo_groups += 1;
        self.undo_group = Some(self.undo_groups);
        work(self);
        self.undo_group = None;
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub(crate) fn undo(&mut self) {
        let Some(edits) = self.history.undo() else {
            return;
        };
        self.timeline.pick(FxHashSet::default());
        for (subject, edit) in edits.into_iter().rev() {
            self.replay(subject, &edit, false);
        }
    }

    pub(crate) fn redo(&mut self) {
        let Some(edits) = self.history.redo() else {
            return;
        };
        self.timeline.pick(FxHashSet::default());
        for (subject, edit) in edits {
            self.replay(subject, &edit, true);
        }
    }

    pub(super) fn film(&mut self, motion: Option<Arc<CameraMotion>>) {
        let motion = motion.filter(|motion| !motion.is_empty());
        self.cast.set_camera(motion.clone().map(CameraTrack::new));
        self.stage.lock().set_camera(motion);
        self.rebake_gaze();
    }

    pub(crate) fn has_picks(&self) -> bool {
        self.timeline.has_picks()
    }

    pub(crate) fn picked_keys(&self) -> Option<(Subject, Vec<Key>)> {
        if !self.timeline.has_picks() {
            return None;
        }
        let (subject, subject_motion) = self.subject_motion()?;
        Some((subject, subject_motion.gather_keys(&self.timeline.picked())))
    }

    pub(crate) fn can_paste(&self) -> bool {
        let Some(clip) = &self.clip else {
            return false;
        };
        let Some((subject, motion)) = self.subject_motion() else {
            return false;
        };
        let human = match subject {
            Subject::Figure(id) => self.cast.figure(id).is_some_and(|figure| figure.is_human()),
            _ => false,
        };
        clip.fits(|kind| motion.accepts(kind, human))
    }

    pub(crate) fn delete_keys(&mut self) {
        if !self.timeline.has_picks()
            && let Some((id, hold)) = self.chosen_hold()
        {
            self.drop_hold(id, hold);
            return;
        }
        let Some((subject, subject_motion)) = self.subject_motion() else {
            return;
        };
        let change = Change::deleting(&subject_motion, &self.timeline.picked());
        self.timeline.pick(FxHashSet::default());
        self.commit(subject, change);
    }

    pub(crate) fn copy_keys(&mut self) {
        let Some((_, subject_motion)) = self.subject_motion() else {
            return;
        };
        let keys = subject_motion.gather_keys(&self.timeline.picked());
        if !keys.is_empty() {
            self.clip = Some(Clip::lifted(keys));
        }
    }

    pub(crate) fn cut_keys(&mut self) {
        self.copy_keys();
        self.delete_keys();
    }

    pub(crate) fn paste_keys(&mut self) {
        if !self.can_paste() {
            return;
        }
        let (Some((subject, subject_motion)), Some(clip)) = (self.subject_motion(), &self.clip)
        else {
            return;
        };
        let keys = clip.laid_at(self.transport.frame());
        let added_ids: FxHashSet<KeyId> = keys.iter().map(Key::id).collect();
        self.commit(subject, Change::laying(&subject_motion, keys));
        self.timeline.pick(added_ids);
    }

    pub(crate) fn shift_keys(&mut self, by: i64) {
        let Some((subject, subject_motion)) = self.subject_motion() else {
            return;
        };
        let change = Change::shifting(&subject_motion, &self.timeline.picked(), by);
        let added_ids: FxHashSet<KeyId> = change.added().iter().map(Key::id).collect();
        self.commit(subject, change);
        self.timeline.pick(added_ids);
    }

    pub(crate) fn lay(&mut self, subject: Subject, keys: Vec<Key>) {
        let Some(motion) = self.motion_of(subject) else {
            return;
        };
        self.commit(subject, Change::laying(&motion, keys));
    }

    pub(crate) fn chosen_bone(&self) -> Option<usize> {
        let Some(Subject::Figure(id)) = self.subject else {
            return None;
        };
        self.timeline.chosen_bone(&self.cast.figure(id)?.skeleton)
    }

    pub(crate) fn has_unregistered(&self) -> bool {
        if self.is_easy_mode() {
            return !self.unregistered_parts().is_empty() || !self.unregistered_morphs().is_empty();
        }
        match self.subject {
            Some(Subject::Figure(id)) => self
                .stage
                .lock()
                .pending_at(id, self.transport.frame())
                .is_some_and(|pending| pending.bones().is_some() || !pending.morphs.is_empty()),
            Some(Subject::Emitter(id)) => self.has_unkeyed(id),
            Some(Subject::Wind) => self.has_unkeyed(Blowing),
            _ => false,
        }
    }

    pub(crate) fn can_register(&self) -> bool {
        if self.is_easy_mode() {
            return !self.transport.is_playing() || self.chosen_part().is_some();
        }
        match self.subject {
            Some(Subject::Camera | Subject::Wind) => true,
            Some(Subject::Emitter(id)) => self.cast.emitter(id).is_some(),
            Some(Subject::Look) => !self.cast.look().motion.tracks().is_empty(),
            Some(Subject::Figure(id)) => {
                let frame = self.transport.frame();
                self.chosen_bone().is_some()
                    || self.chosen_morph().is_some()
                    || self.stage.lock().pending_at(id, frame).is_some()
            }
            None => false,
        }
    }

    pub(crate) fn register(&mut self) {
        if self.transport.is_playing() {
            self.transport.pause();
        }
        let frame = self.transport.frame();
        match self.subject {
            Some(Subject::Figure(_)) if self.is_easy_mode() => self.register_parts(),
            Some(Subject::Figure(id)) => self.register_bones(id, frame),
            Some(Subject::Camera) => {
                let shot = self.stage.lock().current_shot();
                let motion = self.camera_motion().unwrap_or_default();
                let curves = motion
                    .tracks()
                    .key(&(), frame)
                    .map_or(self.curves.camera(), |frame| frame.interpolation);
                let key = Key::Camera(shot.keyed(frame, curves));
                self.lay(Subject::Camera, vec![key]);
            }
            Some(Subject::Look) => self.register_look(),
            Some(Subject::Emitter(id)) => self.register_settings(id, frame),
            Some(Subject::Wind) => self.register_settings(Blowing, frame),
            None => {}
        }
    }

    fn register_bones(&mut self, id: FigureId, frame: u32) {
        let Some(figure) = self.cast.figure(id) else {
            return;
        };
        let skeleton = Arc::clone(&figure.skeleton);
        let take = Arc::clone(&figure.take);
        let morphs = self.morph_keys(id, frame, true);
        let pending = self
            .stage
            .lock()
            .take_pending(id, frame)
            .and_then(|pending| pending.bones().cloned())
            .unwrap_or_default();
        let mut bones: Vec<usize> = pending.keys().copied().collect();
        if let Some(chosen) = self.chosen_bone()
            && !bones.contains(&chosen)
        {
            bones.push(chosen);
        }
        bones.sort_unstable();
        let mut keys: Vec<Key> = bones
            .into_iter()
            .filter_map(|index| {
                let bone = skeleton.bone(index).filter(|bone| !bone.driven)?;
                let local = pending.get(&index).copied().unwrap_or_else(|| {
                    take.bones.value_at(&bone.name, frame as f32).map_or_else(
                        Local::default,
                        |(translation, rotation)| Local {
                            translation,
                            rotation,
                        },
                    )
                });
                let standing = take.bones.key(&bone.name, frame);
                Some(Key::Bone(VmdBoneFrame {
                    bone: bone.name.clone(),
                    frame,
                    translation: local.translation,
                    rotation: local.rotation,
                    interpolation: standing
                        .map_or(self.curves.bone(), |standing| standing.interpolation),
                    physics_off: standing.is_some_and(|standing| standing.physics_off),
                }))
            })
            .collect();
        keys.extend(morphs);
        self.lay(Subject::Figure(id), keys);
    }

    pub(crate) fn can_reset_pose(&self) -> bool {
        if self.is_easy_mode() {
            return !self.transport.is_playing();
        }
        match self.subject {
            Some(Subject::Camera) => true,
            Some(Subject::Look) => false,
            Some(Subject::Emitter(id)) => self.has_unkeyed(id),
            Some(Subject::Wind) => self.has_unkeyed(Blowing),
            Some(Subject::Figure(_)) => {
                self.chosen_bone().is_some() || self.chosen_morph().is_some()
            }
            None => false,
        }
    }

    pub(crate) fn reset_pose(&mut self) {
        match self.subject {
            Some(Subject::Figure(_)) if self.is_easy_mode() => self.reset_parts(),
            Some(Subject::Figure(id)) => {
                if let Some(morph) = self.chosen_morph() {
                    self.zero_morphs(Some(&morph));
                    return;
                }
                let Some(bone) = self.chosen_bone() else {
                    return;
                };
                let frame = self.transport.frame();
                self.stage.lock().hold(id, frame, bone, Local::default());
            }
            Some(Subject::Camera) => {
                self.stage.lock().reframe(|shot| *shot = Shot::default());
            }
            Some(Subject::Emitter(_)) => {
                self.stage.lock().drafts_mut().pour.discard();
            }
            Some(Subject::Wind) => {
                self.stage.lock().drafts_mut().wind.discard();
            }
            Some(Subject::Look) | None => {}
        }
    }

    fn bent_channels(&self) -> Vec<Channel> {
        match self.controls.linked {
            true => self.channels().to_vec(),
            false => vec![self.channel()],
        }
    }
}

impl CurveEditing for Animate {
    fn channel(&self) -> Channel {
        let channels = self.channels();
        match channels.contains(&self.channel) {
            true => self.channel,
            false => channels[0],
        }
    }
    fn channels(&self) -> &'static [Channel] {
        Channel::of(self.keying().unwrap_or(KeyKind::Bone))
    }
    fn choose_channel(&mut self, index: usize) {
        if let Some(channel) = self.channels().get(index) {
            self.channel = *channel;
        }
    }
    fn shown_curve(&self) -> VmdCurve {
        let channel = self.channel();
        if let Some(curve) = self
            .picked_keys()
            .and_then(|(_, keys)| keys.iter().find_map(|key| channel.curve_of(key)))
        {
            return curve;
        }
        self.curves
            .curve(self.keying().unwrap_or(KeyKind::Bone), channel)
    }
    fn bend(&mut self, curve: VmdCurve) {
        let channels = self.bent_channels();
        let channel = self.channel();
        if let Some((subject, keys)) = self.picked_keys()
            && keys.iter().any(|key| channel.curve_of(key).is_some())
        {
            self.commit(subject, Change::bending(keys, &channels, curve));
            return;
        }
        let kind = self.keying().unwrap_or(KeyKind::Bone);
        for channel in channels {
            self.curves.bend(kind, channel, curve);
        }
    }
}
