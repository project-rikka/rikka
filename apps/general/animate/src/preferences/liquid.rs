use animate_fluid::{FluidSettings, Gravity, Wetness};
use rikka_model::glam::Vec3;
use rikka_settings::{Key, Schema};

use super::Preferences;

const PARTICLE_SIZE: &str = "liquid-particle-size";
const CAPACITY: &str = "liquid-capacity";
const GRAVITY: &str = "liquid-gravity";
const WETNESS: &str = "liquid-wetness";
const DRYING: &str = "liquid-drying";
const STRONGEST_PULL: f64 = 50.0;

impl Preferences {
    pub(crate) fn liquid(&self) -> FluidSettings {
        let settings = self.0.borrow();
        let fallback = FluidSettings::default();
        let size = settings.get::<f64>(PARTICLE_SIZE) as f32;
        let capacity = settings.get::<u32>(CAPACITY);
        let pull = settings.get::<f64>(GRAVITY) as f32;
        let drying = settings.get::<f64>(DRYING) as f32;
        FluidSettings {
            particle_size: match FluidSettings::PARTICLE_SIZES.contains(&size) {
                true => size,
                false => fallback.particle_size,
            },
            capacity: match FluidSettings::CAPACITIES.contains(&capacity) {
                true => capacity,
                false => fallback.capacity,
            },
            wetness: match (
                settings.get::<bool>(WETNESS),
                FluidSettings::DRYING_SECONDS.contains(&drying),
            ) {
                (false, _) => Wetness::Off,
                (true, true) => Wetness::On { drying },
                (true, false) => Wetness::default(),
            },
            gravity: pulled(pull),
        }
    }

    pub(crate) fn keep_liquid(&self, settings: &FluidSettings) {
        let mut stored = self.0.borrow_mut();
        stored.set(PARTICLE_SIZE, f64::from(settings.particle_size));
        stored.set(CAPACITY, settings.capacity);
        stored.set(GRAVITY, f64::from(-settings.gravity.vector().y));
        match settings.wetness {
            Wetness::On { drying } => {
                stored.set(WETNESS, true);
                stored.set(DRYING, f64::from(drying));
            }
            Wetness::Off => {
                stored.set(WETNESS, false);
            }
        }
    }
}

fn pulled(pull: f32) -> Gravity {
    match (pull - Gravity::EARTH).abs() < 1e-3 {
        true => Gravity::Earth,
        false => Gravity::Custom(Vec3::new(0.0, -pull, 0.0)),
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    let fallback = FluidSettings::default();
    let sizes = &FluidSettings::PARTICLE_SIZES;
    let capacities = &FluidSettings::CAPACITIES;
    let drying = &FluidSettings::DRYING_SECONDS;
    schema
        .key(
            Key::float(PARTICLE_SIZE, f64::from(fallback.particle_size))
                .range(f64::from(*sizes.start())..=f64::from(*sizes.end()))
                .summary("Diameter of one liquid particle in metres"),
        )
        .key(
            Key::int(CAPACITY, i64::from(fallback.capacity))
                .range(i64::from(*capacities.start())..=i64::from(*capacities.end()))
                .summary("Most liquid particles one scene holds"),
        )
        .key(
            Key::float(GRAVITY, f64::from(Gravity::EARTH))
                .range(0.0..=STRONGEST_PULL)
                .summary("Downward pull on liquids in m/s²"),
        )
        .key(Key::bool(WETNESS, true).summary("Whether liquids leave figures wet"))
        .key(
            Key::float(DRYING, f64::from(FluidSettings::DRYING))
                .range(f64::from(*drying.start())..=f64::from(*drying.end()))
                .summary("Seconds a wet figure takes to dry"),
        )
}
