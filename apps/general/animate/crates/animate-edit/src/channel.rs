use animate_fluid::EmitterKey;
use animate_look::LookKey;
use animate_motion::{MorphKey, PartKey};
use animate_wind::WindKey;
use rikka_mmd::{VmdBoneFrame, VmdCameraFrame, VmdCameraInterpolation, VmdCurve, VmdInterpolation};

use crate::keys::{Key, KeyKind, each_key, map_key};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    X,
    Y,
    Z,
    Rotation,
    Distance,
    ViewAngle,
    Value,
}

impl Channel {
    pub const OF_BONES: [Channel; 4] = [Self::X, Self::Y, Self::Z, Self::Rotation];
    pub const OF_CAMERAS: [Channel; 6] = [
        Self::X,
        Self::Y,
        Self::Z,
        Self::Rotation,
        Self::Distance,
        Self::ViewAngle,
    ];
    pub const OF_VALUES: [Channel; 1] = [Self::Value];

    #[must_use]
    pub fn of(kind: KeyKind) -> &'static [Channel] {
        match kind {
            KeyKind::Bone => &Self::OF_BONES,
            KeyKind::Camera => &Self::OF_CAMERAS,
            KeyKind::Part => &[],
            KeyKind::Morph | KeyKind::Look | KeyKind::Fluid | KeyKind::Wind => &Self::OF_VALUES,
        }
    }

    #[must_use]
    pub fn in_bone(self, curves: &VmdInterpolation) -> Option<VmdCurve> {
        match self {
            Self::X => Some(curves.x),
            Self::Y => Some(curves.y),
            Self::Z => Some(curves.z),
            Self::Rotation => Some(curves.rotation),
            Self::Distance | Self::ViewAngle | Self::Value => None,
        }
    }

    #[must_use]
    pub fn in_camera(self, curves: &VmdCameraInterpolation) -> Option<VmdCurve> {
        match self {
            Self::X => Some(curves.x),
            Self::Y => Some(curves.y),
            Self::Z => Some(curves.z),
            Self::Rotation => Some(curves.rotation),
            Self::Distance => Some(curves.distance),
            Self::ViewAngle => Some(curves.angle),
            Self::Value => None,
        }
    }

    pub fn set_in_bone(self, curves: &mut VmdInterpolation, curve: VmdCurve) {
        match self {
            Self::X => curves.x = curve,
            Self::Y => curves.y = curve,
            Self::Z => curves.z = curve,
            Self::Rotation => curves.rotation = curve,
            Self::Distance | Self::ViewAngle | Self::Value => {}
        }
    }

    pub fn set_in_camera(self, curves: &mut VmdCameraInterpolation, curve: VmdCurve) {
        match self {
            Self::X => curves.x = curve,
            Self::Y => curves.y = curve,
            Self::Z => curves.z = curve,
            Self::Rotation => curves.rotation = curve,
            Self::Distance => curves.distance = curve,
            Self::ViewAngle => curves.angle = curve,
            Self::Value => {}
        }
    }

    #[must_use]
    pub fn curve_of(self, key: &Key) -> Option<VmdCurve> {
        each_key!(key, key => key.curve(self))
    }

    #[must_use]
    pub fn bent(self, key: Key, curve: VmdCurve) -> Key {
        map_key!(key, key => {
            let mut key = key;
            key.bend(self, curve);
            key
        })
    }
}

pub(crate) trait Curved {
    fn curve(&self, channel: Channel) -> Option<VmdCurve>;

    fn bend(&mut self, channel: Channel, curve: VmdCurve);
}

impl Curved for VmdBoneFrame {
    fn curve(&self, channel: Channel) -> Option<VmdCurve> {
        channel.in_bone(&self.interpolation)
    }

    fn bend(&mut self, channel: Channel, curve: VmdCurve) {
        channel.set_in_bone(&mut self.interpolation, curve);
    }
}

impl Curved for VmdCameraFrame {
    fn curve(&self, channel: Channel) -> Option<VmdCurve> {
        channel.in_camera(&self.interpolation)
    }

    fn bend(&mut self, channel: Channel, curve: VmdCurve) {
        channel.set_in_camera(&mut self.interpolation, curve);
    }
}

impl Curved for PartKey {
    fn curve(&self, _: Channel) -> Option<VmdCurve> {
        None
    }

    fn bend(&mut self, _: Channel, _: VmdCurve) {}
}

macro_rules! eased {
    ($($key:ty),+) => {
        $(impl Curved for $key {
            fn curve(&self, channel: Channel) -> Option<VmdCurve> {
                (channel == Channel::Value).then_some(self.curve)
            }

            fn bend(&mut self, channel: Channel, curve: VmdCurve) {
                if channel == Channel::Value {
                    self.curve = curve;
                }
            }
        })+
    };
}

eased!(MorphKey, LookKey, EmitterKey, WindKey);
