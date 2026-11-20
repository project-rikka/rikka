use std::sync::Arc;

use animate_document::Motion;
use animate_edit::{Key, KeyId, KeyKind, Keyed};
use animate_fluid::Emitter;
use animate_look::Look;
use animate_motion::{CameraMotion, Take};
use animate_wind::WindMotion;
use rustc_hash::FxHashSet;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SubjectMotion {
    Take(Arc<Take>),
    Camera(Arc<CameraMotion>),
    Look(Arc<Look>),
    Emitter(Arc<Emitter>),
    Wind(Arc<WindMotion>),
}

macro_rules! each_motion {
    ($motion:expr, $bound:ident => $body:expr) => {
        match $motion {
            SubjectMotion::Take($bound) => $body,
            SubjectMotion::Camera($bound) => $body,
            SubjectMotion::Look($bound) => $body,
            SubjectMotion::Emitter($bound) => $body,
            SubjectMotion::Wind($bound) => $body,
        }
    };
}

pub(crate) trait Subjective: Clone + PartialEq + Sized + 'static {
    fn wrap(motion: Arc<Self>) -> SubjectMotion;

    fn of(motion: &SubjectMotion) -> Option<&Arc<Self>>;
}

macro_rules! subjective {
    ($($motion:ty => $variant:ident;)+) => {
        $(impl Subjective for $motion {
            fn wrap(motion: Arc<Self>) -> SubjectMotion {
                SubjectMotion::$variant(motion)
            }

            fn of(motion: &SubjectMotion) -> Option<&Arc<Self>> {
                match motion {
                    SubjectMotion::$variant(motion) => Some(motion),
                    _ => None,
                }
            }
        })+
    };
}

subjective! {
    Take => Take;
    CameraMotion => Camera;
    Look => Look;
    Emitter => Emitter;
    WindMotion => Wind;
}

impl SubjectMotion {
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::Take(take) => take.is_still(),
            Self::Camera(camera) => camera.is_empty(),
            Self::Look(_) => false,
            Self::Emitter(_) => false,
            Self::Wind(wind) => wind.is_empty(),
        }
    }

    pub(crate) fn document(&self) -> Motion {
        match self {
            Self::Take(take) => Motion::Figure(Box::new((**take).clone())),
            Self::Camera(camera) => Motion::Camera(Box::new((**camera).clone())),
            Self::Look(look) => Motion::Look(Box::new((**look).clone())),
            Self::Emitter(emitter) => Motion::Fluid(Box::new((**emitter).clone())),
            Self::Wind(wind) => Motion::Wind(Box::new((**wind).clone())),
        }
    }

    pub(crate) fn accepts(&self, kind: KeyKind, human: bool) -> bool {
        match (self, kind) {
            (Self::Take(_), KeyKind::Bone | KeyKind::Morph) => true,
            (Self::Take(_), KeyKind::Part) => human,
            (Self::Camera(_), KeyKind::Camera) => true,
            (Self::Look(_), KeyKind::Look) => true,
            (Self::Emitter(_), KeyKind::Fluid) => true,
            (Self::Wind(_), KeyKind::Wind) => true,
            _ => false,
        }
    }
}

impl Keyed for SubjectMotion {
    fn gather_keys(&self, ids: &FxHashSet<KeyId>) -> Vec<Key> {
        each_motion!(self, motion => motion.gather_keys(ids))
    }

    fn remove_keys(&mut self, ids: &FxHashSet<KeyId>) {
        each_motion!(self, motion => motion.remove_keys(ids))
    }

    fn insert_keys(&mut self, keys: &[Key]) {
        each_motion!(self, motion => motion.insert_keys(keys))
    }
}
