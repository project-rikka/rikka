use animate_fluid::EmitterKey;
use animate_look::LookKey;
use animate_motion::{MorphKey, PartKey};
use animate_sheet::Lane;
use animate_track::Keyframe;
use animate_wind::WindKey;
use rikka_keyframe::Framed;
use rikka_mmd::{VmdBoneFrame, VmdCameraFrame};
use rustc_hash::FxHashSet;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyId {
    pub lane: Lane,
    pub frame: u32,
}

impl KeyId {
    #[must_use]
    pub fn new(lane: Lane, frame: u32) -> Self {
        Self { lane, frame }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyKind {
    Bone,
    Morph,
    Camera,
    Part,
    Look,
    Fluid,
    Wind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Bone(VmdBoneFrame),
    Morph(MorphKey),
    Camera(VmdCameraFrame),
    Part(PartKey),
    Look(LookKey),
    Fluid(EmitterKey),
    Wind(WindKey),
}

macro_rules! each_key {
    ($key:expr, $bound:ident => $body:expr) => {
        match $key {
            $crate::keys::Key::Bone($bound) => $body,
            $crate::keys::Key::Morph($bound) => $body,
            $crate::keys::Key::Camera($bound) => $body,
            $crate::keys::Key::Part($bound) => $body,
            $crate::keys::Key::Look($bound) => $body,
            $crate::keys::Key::Fluid($bound) => $body,
            $crate::keys::Key::Wind($bound) => $body,
        }
    };
}

macro_rules! map_key {
    ($key:expr, $bound:ident => $body:expr) => {
        match $key {
            $crate::keys::Key::Bone($bound) => $crate::keys::Key::Bone($body),
            $crate::keys::Key::Morph($bound) => $crate::keys::Key::Morph($body),
            $crate::keys::Key::Camera($bound) => $crate::keys::Key::Camera($body),
            $crate::keys::Key::Part($bound) => $crate::keys::Key::Part($body),
            $crate::keys::Key::Look($bound) => $crate::keys::Key::Look($body),
            $crate::keys::Key::Fluid($bound) => $crate::keys::Key::Fluid($body),
            $crate::keys::Key::Wind($bound) => $crate::keys::Key::Wind($body),
        }
    };
}

pub(crate) use {each_key, map_key};

pub(crate) trait Laned: Keyframe + 'static {
    fn id_in(lane: &Lane) -> Option<&Self::Id>;

    fn lane(id: &Self::Id) -> Lane;

    fn into_key(self) -> Key;

    fn from_key(key: &Key) -> Option<&Self>;
}

macro_rules! laned {
    ($($key:ty => $variant:ident;)+) => {
        $(impl Laned for $key {
            fn id_in(lane: &Lane) -> Option<&Self::Id> {
                match lane {
                    Lane::$variant(id) => Some(id),
                    _ => None,
                }
            }

            fn lane(id: &Self::Id) -> Lane {
                Lane::$variant(id.clone())
            }

            fn into_key(self) -> Key {
                Key::$variant(self)
            }

            fn from_key(key: &Key) -> Option<&Self> {
                match key {
                    Key::$variant(key) => Some(key),
                    _ => None,
                }
            }
        })+
    };
}

laned! {
    VmdBoneFrame => Bone;
    MorphKey => Morph;
    PartKey => Part;
    LookKey => Look;
    EmitterKey => Fluid;
    WindKey => Wind;
}

impl Laned for VmdCameraFrame {
    fn id_in(lane: &Lane) -> Option<&()> {
        matches!(lane, Lane::Camera).then_some(&())
    }

    fn lane((): &()) -> Lane {
        Lane::Camera
    }

    fn into_key(self) -> Key {
        Key::Camera(self)
    }

    fn from_key(key: &Key) -> Option<&Self> {
        match key {
            Key::Camera(key) => Some(key),
            _ => None,
        }
    }
}

fn lane_of<K: Laned>(key: &K) -> Lane {
    K::lane(key.id())
}

impl Key {
    #[must_use]
    pub fn id(&self) -> KeyId {
        KeyId::new(each_key!(self, key => lane_of(key)), self.frame())
    }

    #[must_use]
    pub fn frame(&self) -> u32 {
        each_key!(self, key => key.frame())
    }

    #[must_use]
    pub fn with_frame(self, frame: u32) -> Self {
        map_key!(self, key => key.at_frame(frame))
    }

    #[must_use]
    pub fn kind(&self) -> KeyKind {
        match self {
            Self::Bone(_) => KeyKind::Bone,
            Self::Morph(_) => KeyKind::Morph,
            Self::Camera(_) => KeyKind::Camera,
            Self::Part(_) => KeyKind::Part,
            Self::Look(_) => KeyKind::Look,
            Self::Fluid(_) => KeyKind::Fluid,
            Self::Wind(_) => KeyKind::Wind,
        }
    }
}

pub(crate) fn ids(keys: &[Key]) -> FxHashSet<KeyId> {
    keys.iter().map(Key::id).collect()
}
