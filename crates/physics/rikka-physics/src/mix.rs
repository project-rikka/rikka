use glam::Vec3;
use rikka_physics_fluid::{MAX_SUBSTANCES, Substance};
use rikka_physics_liquid::Liquid;
use rikka_renderer_3d::FluidMaterial;

use crate::scene_source::LiquidEmitter;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mix {
    liquids: Vec<Liquid>,
    emitters: Vec<(u32, u32)>,
}

impl Mix {
    #[must_use]
    pub fn from_emitters(emitters: &[LiquidEmitter]) -> Mix {
        let mut mix = Mix::default();
        for emitter in emitters {
            let index = match mix
                .liquids
                .iter()
                .position(|liquid| *liquid == emitter.liquid)
            {
                Some(index) => index,
                None if mix.liquids.len() < MAX_SUBSTANCES => {
                    mix.liquids.push(emitter.liquid);
                    mix.liquids.len() - 1
                }
                None => MAX_SUBSTANCES - 1,
            };
            mix.emitters.push((emitter.id, index as u32));
        }
        mix
    }

    #[must_use]
    pub(crate) fn substance_of(&self, emitter: u32) -> u32 {
        self.emitters
            .iter()
            .find(|(key, _)| *key == emitter)
            .map_or(0, |(_, index)| *index)
    }

    #[must_use]
    pub(crate) fn substances(&self) -> Vec<Substance> {
        if self.liquids.is_empty() {
            vec![Substance::default()]
        } else {
            self.liquids.iter().map(substance).collect()
        }
    }

    #[must_use]
    pub fn materials(&self) -> Vec<FluidMaterial> {
        self.liquids.iter().map(material).collect()
    }
}

fn substance(liquid: &Liquid) -> Substance {
    Substance {
        viscosity: liquid.viscosity,
        elasticity: liquid.elasticity,
        relaxation: liquid.relaxation,
        cohesion: liquid.cohesion,
        adhesion: liquid.adhesion,
        friction: liquid.friction,
        spray: liquid.spray,
        wetting: liquid.wetting,
        tint: liquid.tint,
    }
}

fn material(liquid: &Liquid) -> FluidMaterial {
    FluidMaterial {
        color: Vec3::from_array(liquid.color),
        depth: liquid.depth,
        cloudiness: liquid.cloudiness,
        roughness: liquid.roughness,
        ior: liquid.ior,
    }
}
