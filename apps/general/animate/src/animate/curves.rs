use animate_edit::{Channel, KeyKind};
use rikka_model::mmd::{VmdCameraInterpolation, VmdCurve, VmdInterpolation};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Curves {
    bone: VmdInterpolation,
    camera: VmdCameraInterpolation,
    morph: VmdCurve,
    look: VmdCurve,
    fluid: VmdCurve,
    wind: VmdCurve,
}

impl Curves {
    pub(crate) fn bone(&self) -> VmdInterpolation {
        self.bone
    }

    pub(crate) fn camera(&self) -> VmdCameraInterpolation {
        self.camera
    }

    pub(crate) fn value(&self, kind: KeyKind) -> VmdCurve {
        self.curve(kind, Channel::Value)
    }

    pub(crate) fn curve(&self, kind: KeyKind, channel: Channel) -> VmdCurve {
        match kind {
            KeyKind::Bone => channel.in_bone(&self.bone).unwrap_or_default(),
            KeyKind::Camera => channel.in_camera(&self.camera).unwrap_or_default(),
            KeyKind::Morph => self.morph,
            KeyKind::Look => self.look,
            KeyKind::Fluid => self.fluid,
            KeyKind::Wind => self.wind,
            KeyKind::Part => VmdCurve::default(),
        }
    }

    pub(crate) fn bend(&mut self, kind: KeyKind, channel: Channel, curve: VmdCurve) {
        match kind {
            KeyKind::Bone => channel.set_in_bone(&mut self.bone, curve),
            KeyKind::Camera => channel.set_in_camera(&mut self.camera, curve),
            KeyKind::Morph => self.morph = curve,
            KeyKind::Look => self.look = curve,
            KeyKind::Fluid => self.fluid = curve,
            KeyKind::Wind => self.wind = curve,
            KeyKind::Part => {}
        }
    }
}
