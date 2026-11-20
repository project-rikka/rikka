use std::hash::Hasher;

use glam::Vec3;
use rustc_hash::FxHasher;

use super::gpu::{GpuBurst, GpuEmitter, GpuEmitterKey};
use crate::error::{Error, Result};
use crate::gpu::NONE_INDEX;
use crate::look::emitter::noise::noise_offset;
use crate::look::{
    Coloring, Emitter, GroundContact, Look, MAX_BURST_COUNT, MAX_BURSTS, ParticleBlend,
    ParticleFacing, ParticleLighting,
};
use rikka_gpu::texture::TexturePool;

pub(crate) const MAX_EMITTER_PARTICLES: u32 = 4_000_000;
pub(crate) const MAX_FRAME_PARTICLES: u32 = 16_000_000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Drawn {
    pub index: u32,
    pub capacity: u32,
    distance: f32,
}

#[derive(Default)]
pub(crate) struct EmitterFrame {
    emitters: Vec<GpuEmitter>,
    keys: Vec<GpuEmitterKey>,
    drawn: Vec<Drawn>,
    textures: Vec<u32>,
    particles: u32,
}

impl EmitterFrame {
    pub(crate) fn resolve(
        &mut self,
        look: Option<&Look>,
        camera: Vec3,
        time: f32,
        textures: &TexturePool,
    ) -> Result<()> {
        self.emitters.clear();
        self.keys.clear();
        self.drawn.clear();
        self.textures.clear();
        self.particles = 0;
        let Some(look) = look else {
            return Ok(());
        };
        for emitter in &look.emitters {
            check(emitter)?;
            let capacity = emitter.capacity();
            if capacity == 0 {
                continue;
            }
            if capacity > MAX_EMITTER_PARTICLES {
                return Err(Error::Invalid(format!(
                    "an emitter holds at most {MAX_EMITTER_PARTICLES} particles, got {capacity}"
                )));
            }
            let first_particle = self.particles;
            self.particles = self
                .particles
                .checked_add(capacity)
                .filter(|&total| total <= MAX_FRAME_PARTICLES)
                .ok_or_else(|| {
                    Error::Invalid(format!(
                        "a frame draws at most {MAX_FRAME_PARTICLES} emitter particles"
                    ))
                })?;
            let (sprite, sampler) = match emitter.appearance.sprite {
                Some(sprite) => {
                    let slot =
                        textures
                            .slot(sprite.texture.texture)
                            .ok_or(rikka_gpu::Error::Unknown(rikka_gpu::Resource::Texture(
                                sprite.texture.texture,
                            )))?;
                    self.textures.push(slot);
                    (slot, sprite.texture.filtering.slot())
                }
                None => (NONE_INDEX, 0),
            };
            let first_key = self.keys.len() as u32;
            self.keys
                .extend(emitter.track.raw().iter().map(|key| GpuEmitterKey {
                    translation: key.translation.to_array(),
                    time: key.time,
                    rotation: key.rotation.to_array(),
                    scale: key.scale.to_array(),
                    pad: 0.0,
                }));
            let index = self.emitters.len() as u32;
            self.emitters.push(pack(
                emitter,
                Packing {
                    first_particle,
                    capacity,
                    first_key,
                    sprite,
                    sampler,
                },
            ));
            self.drawn.push(Drawn {
                index,
                capacity,
                distance: emitter.track.position_at(time).distance(camera),
            });
        }
        self.drawn.sort_unstable_by(|a, b| {
            b.distance
                .total_cmp(&a.distance)
                .then(a.index.cmp(&b.index))
        });
        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.particles == 0
    }

    pub(crate) fn particles(&self) -> u32 {
        self.particles
    }

    pub(crate) fn emitters(&self) -> &[GpuEmitter] {
        &self.emitters
    }

    pub(crate) fn keys(&self) -> &[GpuEmitterKey] {
        &self.keys
    }

    pub(crate) fn drawn(&self) -> &[Drawn] {
        &self.drawn
    }

    pub(crate) fn texture_slots(&self) -> impl Iterator<Item = u32> + '_ {
        self.textures.iter().copied()
    }

    pub(crate) fn hash_into(&self, hasher: &mut FxHasher) {
        hasher.write(bytemuck::cast_slice(&self.emitters));
        hasher.write(bytemuck::cast_slice(&self.keys));
    }
}

fn check(emitter: &Emitter) -> Result<()> {
    let emission = &emitter.emission;
    if emission.bursts.len() > MAX_BURSTS {
        return Err(Error::Invalid(format!(
            "an emitter holds at most {MAX_BURSTS} bursts, got {}",
            emission.bursts.len()
        )));
    }
    if let Some(burst) = emission
        .bursts
        .iter()
        .find(|burst| burst.count > MAX_BURST_COUNT || !burst.time.is_finite())
    {
        return Err(Error::Invalid(format!(
            "emitter burst at {} with {} particles is out of range",
            burst.time, burst.count
        )));
    }
    let timed = [emission.start, emission.rate, emission.lifetime];
    if timed.iter().any(|value| !value.is_finite()) || emission.end.is_nan() {
        return Err(Error::Invalid(
            "emitter start, rate and lifetime must be finite".into(),
        ));
    }
    if let Some(sprite) = emitter.appearance.sprite
        && (sprite.columns == 0 || sprite.rows == 0)
    {
        return Err(Error::Invalid(
            "an emitter sprite needs at least one column and one row".into(),
        ));
    }
    Ok(())
}

struct Packing {
    first_particle: u32,
    capacity: u32,
    first_key: u32,
    sprite: u32,
    sampler: u32,
}

fn pack(emitter: &Emitter, packing: Packing) -> GpuEmitter {
    let emission = &emitter.emission;
    let launch = &emitter.launch;
    let forces = &emitter.forces;
    let look = &emitter.appearance;
    let ground = emitter.ground.unwrap_or_default();
    let attractor = forces.attractor;
    let sprite = look.sprite;
    let mut bursts = [GpuBurst::default(); MAX_BURSTS];
    for (slot, burst) in bursts.iter_mut().zip(&emission.bursts) {
        *slot = GpuBurst {
            time: burst.time,
            duration: burst.duration,
            count: burst.count,
            pad: 0,
        };
    }
    let colors = look.color.colors();
    GpuEmitter {
        first_particle: packing.first_particle,
        slots: emission.slots(),
        capacity: packing.capacity,
        seed: emitter.seed,
        first_key: packing.first_key,
        key_count: emitter.track.len() as u32,
        burst_count: emission.bursts.len() as u32,
        shape: emitter.shape.code(),
        start: emission.start,
        end: emission.end,
        rate: emission.rate,
        lifetime: emission.lifetime,
        lifetime_variance: emission.lifetime_variance,
        speed: launch.speed,
        speed_variance: launch.speed_variance,
        inherit: launch.inherit,
        launch: launch.velocity.to_array(),
        radius: emitter.shape.radius(),
        extent: emitter.shape.extent().to_array(),
        angle: emitter.shape.angle(),
        gravity: forces.gravity.to_array(),
        drag: forces.drag,
        wind: forces.wind.to_array(),
        drag_variance: forces.drag_variance,
        noise_offset: noise_offset(emitter.seed).to_array(),
        turbulence: forces.turbulence.strength,
        attractor: attractor.map_or([0.0; 3], |attractor| attractor.position.to_array()),
        attraction: attractor.map_or(0.0, |attractor| attractor.strength),
        frequency: forces.turbulence.frequency,
        flow: forces.turbulence.flow,
        attraction_rate: attractor.map_or(0.0, |attractor| attractor.rate),
        ground: match emitter.ground.map(|ground| ground.contact) {
            None => 0,
            Some(GroundContact::Bounce) => 1,
            Some(GroundContact::Die) => 2,
        },
        ground_height: ground.height,
        restitution: ground.restitution,
        friction: ground.friction,
        ground_fade: ground.fade,
        size_variance: look.size_variance,
        rotation: look.rotation,
        spin: look.spin,
        spin_variance: look.spin_variance,
        stretch: look.stretch.seconds,
        stretch_min: look.stretch.min,
        stretch_max: look.stretch.max,
        softness: look.softness,
        twinkle: look.twinkle.depth,
        twinkle_rate: look.twinkle.rate,
        translucency: match emitter.lighting {
            ParticleLighting::Unlit => 0.0,
            ParticleLighting::Lit { translucency } => translucency,
        },
        layers: emitter.layers,
        axis: match look.facing {
            ParticleFacing::Axis(axis) => axis.to_array(),
            _ => [0.0, 1.0, 0.0],
        },
        facing: look.facing.code(),
        sprite: packing.sprite,
        sampler: packing.sampler,
        columns: sprite.map_or(1, |sprite| sprite.columns),
        rows: sprite.map_or(1, |sprite| sprite.rows),
        frame_rate: sprite.map_or(0.0, |sprite| sprite.frame_rate),
        random_frame: sprite.map_or(0, |sprite| u32::from(sprite.random_start)),
        blend: match look.blend {
            ParticleBlend::Alpha => 0,
            ParticleBlend::Additive => 1,
            ParticleBlend::Premultiplied => 2,
        },
        lit: u32::from(matches!(emitter.lighting, ParticleLighting::Lit { .. })),
        bloom: u32::from(emitter.bloom),
        look: look.shape.code(),
        coloring: match look.coloring {
            Coloring::OverAge => 0,
            Coloring::PerParticle => 1,
        },
        aspect: look.aspect,
        size_ages: look.size.ages(),
        size_values: look.size.values(),
        opacity_ages: look.opacity.ages(),
        opacity_values: look.opacity.values(),
        emission_ages: look.emission.ages(),
        emission_values: look.emission.values(),
        color_ages: look.color.ages(),
        colors: colors.map(|color| color.extend(0.0).to_array()),
        bursts,
    }
}
