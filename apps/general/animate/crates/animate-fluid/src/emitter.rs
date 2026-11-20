use animate_track::Tracks;
use rikka_mmd::FRAMES_PER_SECOND;
use rikka_model_core::glam::Vec3;

use crate::aim::Aim;
use crate::anchor::Spot;
use crate::formula::Formula;
use crate::key::EmitterKey;
use crate::levels::EmitterLevels;
use crate::pour::Pour;
use crate::setting::EmitterSetting;

const SIMPSON_STEPS: usize = 8;
use crate::shape::Shape;
use crate::state::EmitterState;
use crate::value::EmitterValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EmitterId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    pub id: EmitterId,
    pub name: String,
    pub shape: Shape,
    pub aim: Aim,
    pub launch: Spot,
    pub target: Spot,
    pub levels: EmitterLevels,
    pub liquid: Formula,
    pub pour: Pour,
    tracks: Tracks<EmitterKey>,
}

impl Emitter {
    pub const LAUNCH: Vec3 = Vec3::new(0.0, 1.2, 0.0);
    pub const TARGET: Vec3 = Vec3::new(0.0, 0.0, 0.6);

    #[must_use]
    pub fn spot(&self, setting: EmitterSetting) -> Option<&Spot> {
        match setting {
            EmitterSetting::Launch => Some(&self.launch),
            EmitterSetting::Target => Some(&self.target),
            _ => None,
        }
    }

    pub fn spot_mut(&mut self, setting: EmitterSetting) -> Option<&mut Spot> {
        match setting {
            EmitterSetting::Launch => Some(&mut self.launch),
            EmitterSetting::Target => Some(&mut self.target),
            _ => None,
        }
    }

    #[must_use]
    pub fn new(id: EmitterId, name: impl Into<String>, shape: Shape) -> Self {
        Self {
            id,
            name: name.into(),
            shape,
            aim: Aim::default(),
            launch: Spot {
                offset: Self::LAUNCH,
                ..Spot::default()
            },
            target: Spot {
                offset: Self::TARGET,
                ..Spot::default()
            },
            levels: EmitterLevels::default(),
            liquid: Formula::default(),
            pour: Pour::default(),
            tracks: Tracks::default(),
        }
    }

    #[must_use]
    pub fn applies(&self, setting: EmitterSetting) -> bool {
        setting.applies(&self.shape, self.aim)
    }

    pub fn rows(&self) -> impl Iterator<Item = EmitterSetting> + '_ {
        EmitterSetting::ALL
            .iter()
            .copied()
            .filter(|setting| self.applies(*setting))
    }

    pub fn keyed(&self) -> impl Iterator<Item = EmitterSetting> + '_ {
        self.tracks
            .keyed()
            .copied()
            .filter(|setting| self.applies(*setting))
    }

    #[must_use]
    pub fn tracks(&self) -> &Tracks<EmitterKey> {
        &self.tracks
    }

    pub fn tracks_mut(&mut self) -> &mut Tracks<EmitterKey> {
        &mut self.tracks
    }

    #[must_use]
    pub fn last_frame(&self) -> u32 {
        self.keyed()
            .filter_map(|setting| self.tracks.last_frame_of(&setting))
            .chain([self.stops()])
            .max()
            .unwrap_or(0)
    }
    #[must_use]
    pub fn stops(&self) -> u32 {
        match self.shape.flows() {
            true => self.pour.end(),
            false => self.pour.start(),
        }
    }

    #[must_use]
    pub fn resting(&self, setting: EmitterSetting) -> EmitterValue {
        match setting {
            EmitterSetting::Launch => EmitterValue::Three(self.launch.offset.to_array()),
            EmitterSetting::Target => EmitterValue::Three(self.target.offset.to_array()),
            _ => EmitterValue::Number(self.levels.get(setting).unwrap_or_default()),
        }
    }

    pub fn set_resting(&mut self, setting: EmitterSetting, value: EmitterValue) {
        match (setting, value) {
            (EmitterSetting::Launch, EmitterValue::Three(three)) => {
                self.launch.offset = Vec3::from_array(three)
            }
            (EmitterSetting::Target, EmitterValue::Three(three)) => {
                self.target.offset = Vec3::from_array(three)
            }
            (_, EmitterValue::Number(level)) => self.levels.set(setting, level),
            _ => {}
        }
    }

    #[must_use]
    pub fn value_at(&self, setting: EmitterSetting, frame: f32) -> EmitterValue {
        self.tracks
            .value_at(&setting, frame)
            .unwrap_or_else(|| self.resting(setting))
    }

    #[must_use]
    pub fn pours_at(&self, frame: f32) -> bool {
        match self.shape.flows() {
            true => self.pour.holds(frame),
            false => self.pour.start() as f32 <= frame,
        }
    }

    #[must_use]
    pub fn sample(&self, frame: f32) -> EmitterState {
        let point = |setting: EmitterSetting, spot: &Spot| Spot {
            anchor: spot.anchor.clone(),
            offset: self
                .value_at(setting, frame)
                .three()
                .map_or(spot.offset, Vec3::from_array),
        };
        let mut levels = self.levels;
        for setting in self.keyed() {
            if let Some(level) = self.value_at(setting, frame).number() {
                levels.set(setting, level);
            }
        }
        EmitterState {
            on: self.pours_at(frame),
            shape: self.shape.clone(),
            aim: self.aim,
            launch: point(EmitterSetting::Launch, &self.launch),
            target: point(EmitterSetting::Target, &self.target),
            levels,
        }
    }

    #[must_use]
    pub fn lands_between(&self, after: f32, through: f32) -> bool {
        let start = self.pour.start() as f32;
        matches!(self.shape, Shape::Blob { .. }) && after < start && start <= through
    }

    #[must_use]
    pub fn litres(&self) -> f32 {
        let level = |setting: EmitterSetting, frame: f32| {
            self.value_at(setting, frame).number().unwrap_or_default()
        };
        if !self.shape.flows() {
            return level(EmitterSetting::Volume, self.pour.start() as f32);
        }
        if self.tracks.keys(&EmitterSetting::Rate).is_empty() {
            return level(EmitterSetting::Rate, 0.0) * self.pour.seconds();
        }
        let (start, end) = (self.pour.start() as f32, self.pour.end() as f32);
        let inside = self
            .tracks
            .keys(&EmitterSetting::Rate)
            .iter()
            .map(|key| key.frame as f32)
            .filter(|frame| start < *frame && *frame < end);
        let edges: Vec<f32> = std::iter::once(start)
            .chain(inside)
            .chain(std::iter::once(end))
            .collect();
        let rate = |frame: f32| level(EmitterSetting::Rate, frame);
        let flowed: f32 = edges
            .windows(2)
            .map(|piece| simpson(&rate, piece[0], piece[1]))
            .sum();
        flowed / FRAMES_PER_SECOND
    }
}

fn simpson(rate: &impl Fn(f32) -> f32, from: f32, to: f32) -> f32 {
    let step = (to - from) / SIMPSON_STEPS as f32;
    let weighted: f32 = (0..=SIMPSON_STEPS)
        .map(|at| {
            let weight = match at {
                0 => 1.0,
                at if at == SIMPSON_STEPS => 1.0,
                at if at % 2 == 1 => 4.0,
                _ => 2.0,
            };
            weight * rate(from + step * at as f32)
        })
        .sum();
    weighted * step / 3.0
}
