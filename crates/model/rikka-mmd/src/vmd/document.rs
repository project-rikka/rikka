use rikka_model_core::glam::{Quat, Vec2, Vec3};
use rustc_hash::FxHashMap;

use rikka_model_core::Conversion;
use rikka_model_core::Extension;

use crate::euler;

pub const FRAMES_PER_SECOND: f32 = 30.0;

pub const CAMERA_MOTION: &str = "カメラ・照明";

pub(crate) const SIGNATURE: &[u8] = b"Vocaloid Motion Data 0002";
pub(crate) const LEGACY_SIGNATURE: &[u8] = b"Vocaloid Motion Data file";
pub(crate) const HEADER: usize = 30;
pub(crate) const NAME: usize = 20;
pub(crate) const LEGACY_NAME: usize = 10;
pub(crate) const FRAME_NAME: usize = 15;
pub(crate) const IK_NAME: usize = 20;

pub(crate) const PHYSICS_OFF: u16 = 0x0f63;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmdCurve {
    pub first: (u8, u8),
    pub second: (u8, u8),
}

impl Default for VmdCurve {
    fn default() -> Self {
        Self::LINEAR
    }
}

impl VmdCurve {
    pub const LINEAR: Self = Self {
        first: (20, 20),
        second: (107, 107),
    };

    pub const SPAN: u8 = 127;
    const HALVINGS: usize = 12;

    #[must_use]
    pub fn is_linear(self) -> bool {
        self.first.0 == self.first.1 && self.second.0 == self.second.1
    }

    #[must_use]
    pub fn factor(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if self.is_linear() {
            return t;
        }
        let span = f32::from(Self::SPAN);
        let handles = (
            Vec2::new(f32::from(self.first.0), f32::from(self.first.1)) / span,
            Vec2::new(f32::from(self.second.0), f32::from(self.second.1)) / span,
        );
        let mut at = 0.5;
        let mut step = 0.5;
        for _ in 0..Self::HALVINGS {
            step *= 0.5;
            let x = bezier(handles.0.x, handles.1.x, at);
            if x == t {
                break;
            }
            match t <= x {
                true => at -= step,
                false => at += step,
            }
        }
        bezier(handles.0.y, handles.1.y, at)
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            first: (bytes[0], bytes[1]),
            second: (bytes[2], bytes[3]),
        }
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; 4] {
        [self.first.0, self.first.1, self.second.0, self.second.1]
    }
}

fn bezier(first: f32, second: f32, t: f32) -> f32 {
    let inverse = 1.0 - t;
    3.0 * inverse * inverse * t * first + 3.0 * inverse * t * t * second + t * t * t
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VmdInterpolation {
    pub x: VmdCurve,
    pub y: VmdCurve,
    pub z: VmdCurve,
    pub rotation: VmdCurve,
}

impl VmdInterpolation {
    #[must_use]
    pub fn is_linear(self) -> bool {
        self.x.is_linear() && self.y.is_linear() && self.z.is_linear() && self.rotation.is_linear()
    }

    pub(crate) fn curves(&self) -> [VmdCurve; 4] {
        [self.x, self.y, self.z, self.rotation]
    }

    pub(crate) fn from_curves(curves: [VmdCurve; 4]) -> Self {
        Self {
            x: curves[0],
            y: curves[1],
            z: curves[2],
            rotation: curves[3],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VmdCameraInterpolation {
    pub x: VmdCurve,
    pub y: VmdCurve,
    pub z: VmdCurve,
    pub rotation: VmdCurve,
    pub distance: VmdCurve,
    pub angle: VmdCurve,
}

impl VmdCameraInterpolation {
    pub(crate) fn curves(&self) -> [VmdCurve; 6] {
        [
            self.x,
            self.y,
            self.z,
            self.rotation,
            self.distance,
            self.angle,
        ]
    }

    pub(crate) fn from_curves(curves: [VmdCurve; 6]) -> Self {
        Self {
            x: curves[0],
            y: curves[1],
            z: curves[2],
            rotation: curves[3],
            distance: curves[4],
            angle: curves[5],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VmdBoneFrame {
    pub bone: String,
    pub frame: u32,
    pub translation: Vec3,
    pub rotation: Quat,
    pub interpolation: VmdInterpolation,
    pub physics_off: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VmdMorphFrame {
    pub morph: String,
    pub frame: u32,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VmdCameraFrame {
    pub frame: u32,
    pub distance: f32,
    pub target: Vec3,
    pub rotation: Vec3,
    pub interpolation: VmdCameraInterpolation,
    pub angle: u32,
    pub perspective: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VmdLightFrame {
    pub frame: u32,
    pub color: Vec3,
    pub direction: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VmdShadowFrame {
    pub frame: u32,
    pub mode: u8,
    pub distance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VmdPropertyFrame {
    pub frame: u32,
    pub visible: bool,
    pub ik: Vec<(String, bool)>,
}

#[derive(Clone, Debug, Default)]
pub struct Vmd {
    pub model: String,
    pub bones: Vec<VmdBoneFrame>,
    pub morphs: Vec<VmdMorphFrame>,
    pub cameras: Vec<VmdCameraFrame>,
    pub lights: Vec<VmdLightFrame>,
    pub shadows: Vec<VmdShadowFrame>,
    pub properties: Vec<VmdPropertyFrame>,
}

impl Extension for Vmd {
    const NAME: &'static str = "VMD";

    fn convert(&mut self, conversion: &Conversion) {
        for frame in &mut self.bones {
            frame.translation = conversion.point(frame.translation);
            frame.rotation = conversion.rotation(frame.rotation);
        }
        for frame in &mut self.cameras {
            frame.target = conversion.point(frame.target);
            frame.distance *= conversion.scale();
            frame.rotation = euler::angles(conversion, frame.rotation);
        }
        for frame in &mut self.lights {
            frame.direction = conversion.vector(frame.direction);
        }
        for frame in &mut self.shadows {
            frame.distance *= conversion.scale();
        }
    }
}

impl Vmd {
    #[must_use]
    pub fn last_frame(&self) -> u32 {
        let bones = self.bones.iter().map(|frame| frame.frame);
        let morphs = self.morphs.iter().map(|frame| frame.frame);
        let cameras = self.cameras.iter().map(|frame| frame.frame);
        let lights = self.lights.iter().map(|frame| frame.frame);
        let shadows = self.shadows.iter().map(|frame| frame.frame);
        let properties = self.properties.iter().map(|frame| frame.frame);
        bones
            .chain(morphs)
            .chain(cameras)
            .chain(lights)
            .chain(shadows)
            .chain(properties)
            .max()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn duration(&self) -> f32 {
        self.last_frame() as f32 / FRAMES_PER_SECOND
    }
    #[must_use]
    pub fn bones_by_name(&self) -> Vec<(&str, Vec<&VmdBoneFrame>)> {
        grouped(
            self.bones.iter().map(|frame| (frame.bone.as_str(), frame)),
            |frame| frame.frame,
        )
    }

    #[must_use]
    pub fn morphs_by_name(&self) -> Vec<(&str, Vec<&VmdMorphFrame>)> {
        grouped(
            self.morphs
                .iter()
                .map(|frame| (frame.morph.as_str(), frame)),
            |frame| frame.frame,
        )
    }

    pub fn sort(&mut self) {
        self.bones
            .sort_by(|one, two| one.bone.cmp(&two.bone).then(one.frame.cmp(&two.frame)));
        self.morphs
            .sort_by(|one, two| one.morph.cmp(&two.morph).then(one.frame.cmp(&two.frame)));
        self.cameras.sort_by_key(|frame| frame.frame);
        self.lights.sort_by_key(|frame| frame.frame);
        self.shadows.sort_by_key(|frame| frame.frame);
        self.properties.sort_by_key(|frame| frame.frame);
    }

    pub fn merge(&mut self, other: Vmd) {
        if self.model.is_empty() {
            self.model = other.model;
        }
        overlay(&mut self.bones, other.bones, |frame| {
            (frame.bone.clone(), frame.frame)
        });
        overlay(&mut self.morphs, other.morphs, |frame| {
            (frame.morph.clone(), frame.frame)
        });
        overlay(&mut self.cameras, other.cameras, |frame| frame.frame);
        overlay(&mut self.lights, other.lights, |frame| frame.frame);
        overlay(&mut self.shadows, other.shadows, |frame| frame.frame);
        overlay(&mut self.properties, other.properties, |frame| frame.frame);
    }
}

fn overlay<T, K: Eq + std::hash::Hash>(frames: &mut Vec<T>, laid: Vec<T>, key: impl Fn(&T) -> K) {
    if laid.is_empty() {
        return;
    }
    let covered: rustc_hash::FxHashSet<K> = laid.iter().map(&key).collect();
    frames.retain(|frame| !covered.contains(&key(frame)));
    frames.extend(laid);
}

fn grouped<'a, T>(
    frames: impl Iterator<Item = (&'a str, &'a T)>,
    at: impl Fn(&T) -> u32,
) -> Vec<(&'a str, Vec<&'a T>)> {
    let mut groups: Vec<(&str, Vec<&T>)> = Vec::new();
    let mut where_of: FxHashMap<&str, usize> = FxHashMap::default();
    for (name, frame) in frames {
        match where_of.get(name) {
            Some(index) => groups[*index].1.push(frame),
            None => {
                where_of.insert(name, groups.len());
                groups.push((name, vec![frame]));
            }
        }
    }
    for (_, frames) in &mut groups {
        frames.sort_by_key(|frame| at(frame));
    }
    groups
}
