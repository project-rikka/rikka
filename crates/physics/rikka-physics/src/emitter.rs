use glam::Vec3;
use rikka_physics_fluid::{Emission, Nozzle, RegionId};

const SURFACE_SPEED: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterShape {
    Stream { radius: f32 },
    Shower { radius: f32, spread: f32 },
    Drip { radius: f32 },
    Blob,
    Surface,
}

impl EmitterShape {
    fn is_launched(self) -> bool {
        !matches!(self, EmitterShape::Surface)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    pub position: Vec3,
    pub velocity: Option<Vec3>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitterState {
    pub id: u32,
    pub shape: EmitterShape,
    pub enabled: bool,
    pub rate: f32,
    pub volume: f32,
    pub launch: Option<Launch>,
    pub bursts: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EmitterContext {
    pub(crate) substance: u32,
    pub(crate) gravity: Vec3,
    pub(crate) region: Option<RegionId>,
}

impl EmitterContext {
    pub(crate) fn flow(&self, start: &EmitterState, end: &EmitterState) -> Option<Emission> {
        if !start.enabled || start.rate <= 0.0 {
            return None;
        }
        let aimed = if start.shape.is_launched() {
            Some(start.launch?.velocity?)
        } else {
            None
        };
        let (nozzle, velocity) = match start.shape {
            EmitterShape::Stream { radius } => {
                let (radius, velocity) =
                    jet(radius, start.rate, aimed.unwrap_or_default(), self.gravity);
                (Nozzle::Stream { radius }, velocity)
            }
            EmitterShape::Shower { radius, spread } => {
                (Nozzle::Shower { radius, spread }, aimed.unwrap_or_default())
            }
            EmitterShape::Drip { radius } => (Nozzle::Drip { radius }, aimed.unwrap_or_default()),
            EmitterShape::Surface => (
                Nozzle::Surface {
                    region: self.region?,
                    speed: SURFACE_SPEED,
                },
                Vec3::ZERO,
            ),
            EmitterShape::Blob => return None,
        };
        let emission = Emission::new(start.id, nozzle)
            .rate(start.rate)
            .substance(self.substance);
        let Some(from) = start
            .launch
            .filter(|_| start.shape.is_launched())
            .map(|launch| launch.position)
        else {
            return Some(emission);
        };
        let to = end.launch.map_or(from, |launch| launch.position);
        Some(emission.moving(from, to).velocity(velocity))
    }

    pub(crate) fn bursts(&self, end: &EmitterState) -> impl Iterator<Item = Emission> {
        let landed = end
            .launch
            .and_then(|launch| Some((launch.position, launch.velocity?)));
        let count = landed.map_or(0, |_| end.bursts);
        let substance = self.substance;
        let emitter = end.id;
        let volume = end.volume;
        (0..count).filter_map(move |_| {
            let (at, velocity) = landed?;
            Some(
                Emission::new(emitter, Nozzle::Blob)
                    .at(at)
                    .velocity(velocity)
                    .volume(volume)
                    .substance(substance),
            )
        })
    }
}

fn jet(nozzle_radius: f32, rate: f32, aimed: Vec3, gravity: Vec3) -> (f32, Vec3) {
    let outflow =
        rate / (std::f32::consts::PI * nozzle_radius * nozzle_radius).max(f32::MIN_POSITIVE);
    let speed = aimed.length();
    let velocity = if speed >= outflow {
        aimed
    } else {
        aimed
            .try_normalize()
            .unwrap_or(gravity.normalize_or(Vec3::NEG_Y))
            * outflow
    };
    let radius = (rate / (std::f32::consts::PI * velocity.length())).sqrt();
    (radius.min(nozzle_radius), velocity)
}
