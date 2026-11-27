use animate_inspector::MorphEditing;
use std::sync::Arc;

use animate_cast::{Subject, WindTrack};
use animate_edit::{Change, KeyKind};

use crate::animate::Animate;
use crate::animate::edit::{Edit, Pair};
use crate::animate::subject_motion::{SubjectMotion, Subjective};

impl Animate {
    pub(crate) fn motion_of(&self, subject: Subject) -> Option<SubjectMotion> {
        Some(match subject {
            Subject::Figure(id) => SubjectMotion::Take(Arc::clone(&self.cast.figure(id)?.take)),
            Subject::Camera => SubjectMotion::Camera(self.camera_motion().unwrap_or_default()),
            Subject::Look => SubjectMotion::Look(Arc::clone(&self.cast.look().motion)),
            Subject::Emitter(id) => SubjectMotion::Emitter(Arc::clone(self.cast.emitter(id)?)),
            Subject::Wind => SubjectMotion::Wind(Arc::clone(&self.cast.wind().motion)),
            Subject::Stage | Subject::Accessory(_) => {
                SubjectMotion::Prop(Arc::clone(&self.cast.prop(subject)?.prop))
            }
        })
    }

    pub(super) fn put(&mut self, subject: Subject, motion: Option<SubjectMotion>) {
        if subject == Subject::Look {
            let look = match motion {
                Some(SubjectMotion::Look(look)) => look,
                _ => Arc::default(),
            };
            self.install_look(look);
            return;
        }
        match (subject, motion) {
            (Subject::Figure(id), Some(SubjectMotion::Take(take))) => self.set_take(id, take),
            (Subject::Camera, Some(SubjectMotion::Camera(camera))) => self.film(Some(camera)),
            (Subject::Camera, None) => self.film(None),
            (Subject::Wind, motion) => self.cast.set_wind(match motion {
                Some(SubjectMotion::Wind(wind)) => WindTrack::new(wind),
                _ => WindTrack::default(),
            }),
            (Subject::Emitter(id), Some(SubjectMotion::Emitter(emitter))) => {
                let arriving = self.cast.emitter(id).is_none();
                self.cast.place_emitter(emitter);
                if arriving {
                    self.subject = Some(subject);
                    self.timeline.pick(Default::default());
                }
            }
            (Subject::Emitter(id), None) => {
                let next = self.cast.drop_emitter(id);
                if self.subject == Some(subject) {
                    self.subject = next;
                }
            }
            (Subject::Stage, motion) => {
                let prop = match motion {
                    Some(SubjectMotion::Prop(prop)) => Some(prop),
                    _ => None,
                };
                let leaving = prop.is_none();
                self.cast.set_stage(prop);
                if leaving && self.subject == Some(subject) {
                    self.subject = Some(self.cast.fallback());
                }
            }
            (Subject::Accessory(_), Some(SubjectMotion::Prop(prop))) => {
                self.cast.place_accessory(prop);
            }
            (Subject::Accessory(id), None) => {
                let next = self.cast.drop_accessory(id);
                if self.subject == Some(subject) {
                    self.subject = next;
                }
            }
            (subject, motion) => unreachable!("{subject:?} cannot hold {motion:?}"),
        }
        self.refresh();
    }

    pub(super) fn replace(&mut self, subject: Subject, after: Option<SubjectMotion>) {
        let before = self.motion_of(subject);
        if before == after {
            return;
        }
        self.put(subject, after.clone());
        self.record(subject, Edit::Replaced(Pair::new(before, after)));
    }

    pub(super) fn update<M: Subjective>(&mut self, subject: Subject, change: impl FnOnce(&mut M)) {
        let Some(before) = self.motion_of(subject) else {
            return;
        };
        let Some(current) = M::of(&before) else {
            unreachable!("{subject:?} holds {before:?}");
        };
        let mut after = (**current).clone();
        change(&mut after);
        if after != **current {
            self.replace(subject, Some(M::wrap(Arc::new(after))));
        }
    }

    pub(crate) fn rewrite(&mut self, subject: Subject, edit: impl FnOnce(&mut SubjectMotion)) {
        let Some(mut motion) = self.motion_of(subject) else {
            return;
        };
        edit(&mut motion);
        self.put(subject, Some(motion));
    }

    pub(super) fn commit(&mut self, subject: Subject, change: Change) {
        if change.is_empty() {
            return;
        }
        self.rewrite(subject, |motion| change.apply(motion));
        self.record(subject, Edit::Keys(change));
    }

    pub(super) fn replay(&mut self, subject: Subject, edit: &Edit, forward: bool) {
        match edit {
            Edit::Keys(change) => self.rewrite(subject, |motion| match forward {
                true => change.apply(motion),
                false => change.revert(motion),
            }),
            Edit::Replaced(pair) => self.put(subject, pair.toward(forward).clone()),
            Edit::Cloth(pair) => {
                if let Subject::Figure(id) = subject {
                    self.set_cloth_settings(id, Arc::clone(pair.toward(forward)));
                }
            }
        }
    }

    pub(crate) fn keying(&self) -> Option<KeyKind> {
        Some(match self.subject? {
            Subject::Figure(_) if self.chosen_morph().is_some() => KeyKind::Morph,
            Subject::Figure(_) => KeyKind::Bone,
            Subject::Camera => KeyKind::Camera,
            Subject::Look => KeyKind::Look,
            Subject::Emitter(_) => KeyKind::Fluid,
            Subject::Wind => KeyKind::Wind,
            Subject::Stage | Subject::Accessory(_) => KeyKind::Prop,
        })
    }
}
