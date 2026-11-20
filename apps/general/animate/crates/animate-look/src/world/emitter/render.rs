use rikka_renderer_3d::glam::Vec3;
use rikka_renderer_3d::{
    Appearance, Burst, Curve, Emission, Emitter, EmitterShape, EmitterTrack, Forces, Gradient,
    NITS_PER_RADIANCE_UNIT, ParticleFacing, ParticleLighting, Sprite, TextureKind,
};

use crate::asset::Asset;
use crate::world::emitter::{
    AppearanceLook, BurstLook, CustomEmitter, EmitterLook, EmitterPreset, EmitterSource, Facing,
    PresetEmitter, ShapeKind, Spread,
};
use crate::world::render::Resolver;

impl EmitterLook {
    pub fn emitter(&self, resolver: &mut impl Resolver) -> Emitter {
        let base = match &self.source {
            EmitterSource::Preset(preset) => preset.emitter(),
            EmitterSource::Custom(custom) => custom.emitter(custom.sprite(resolver)),
        };
        if self.track.len() < 2 {
            return base.at(self.placement.matrix());
        }
        let offset = base.track.transform_at(0.0);
        let track = EmitterTrack::keyed(
            self.track
                .iter()
                .map(|(time, placement)| (*time, placement.matrix() * offset)),
        )
        .expect("a sampled emitter track has distinct, finite and bounded keys");
        Emitter { track, ..base }
    }

    pub(crate) fn sprite(&self) -> Option<&Asset> {
        match &self.source {
            EmitterSource::Custom(custom) => custom.sprite.texture.as_ref(),
            EmitterSource::Preset(_) => None,
        }
    }

    pub(crate) fn lifetime(&self) -> f32 {
        match &self.source {
            EmitterSource::Preset(preset) => preset.emitter().emission.lifetime,
            EmitterSource::Custom(custom) => custom.emission.lifetime,
        }
    }

    pub(crate) fn start(&self) -> f32 {
        match &self.source {
            EmitterSource::Preset(preset) => preset.timing.start,
            EmitterSource::Custom(custom) => custom.emission.timing.start,
        }
    }
}

impl PresetEmitter {
    pub(crate) fn emitter(&self) -> Emitter {
        let (area, reach) = match self.spread {
            Spread::Area(area) => (area, area.x),
            Spread::Radius(reach) | Spread::Speed(reach) => (Vec3::splat(reach), reach),
        };
        let (amount, wind) = (self.amount, self.wind);
        let made = match self.preset {
            EmitterPreset::Rain => Emitter::rain(amount, area, wind),
            EmitterPreset::Snow => Emitter::snow(amount, area, wind),
            EmitterPreset::Sparks => Emitter::sparks(amount, reach, wind),
            EmitterPreset::Embers => Emitter::embers(amount, reach, wind),
            EmitterPreset::Petals => Emitter::petals(amount, area, wind),
            EmitterPreset::Fireflies => Emitter::fireflies(amount, area, wind),
            EmitterPreset::Dust => Emitter::dust(amount, reach, wind),
            EmitterPreset::Magic => Emitter::magic(amount, reach, wind),
            EmitterPreset::Confetti => Emitter::confetti(amount, area, wind),
            EmitterPreset::Leaves => Emitter::leaves(amount, area, wind),
        };
        let mut made = made.seeded(self.seed);
        made.appearance.color = made.appearance.color.tinted(self.tint);
        made.emission.start = self.timing.start;
        made.emission.end = self.timing.end();
        made
    }
}

impl CustomEmitter {
    fn sprite(&self, resolver: &mut impl Resolver) -> Option<Sprite> {
        let sprite = &self.sprite;
        let texture = resolver.texture(sprite.texture.as_ref()?, TextureKind::Colour)?;
        Some(Sprite {
            random_start: sprite.random_start,
            ..Sprite::new(texture).flipbook(sprite.columns, sprite.rows, sprite.frame_rate)
        })
    }

    pub(crate) fn emitter(&self, sprite: Option<Sprite>) -> Emitter {
        let shape = &self.shape;
        let emission = &self.emission;
        let forces = &self.forces;
        Emitter {
            seed: self.seed,
            shape: match shape.kind {
                ShapeKind::Point => EmitterShape::Point,
                ShapeKind::Sphere => EmitterShape::Sphere {
                    radius: shape.radius,
                },
                ShapeKind::Box => EmitterShape::Box {
                    half_extents: shape.extent,
                },
                ShapeKind::Disc => EmitterShape::Disc {
                    radius: shape.radius,
                },
                ShapeKind::Cone => EmitterShape::Cone {
                    radius: shape.radius,
                    angle: shape.angle,
                },
                ShapeKind::Line => EmitterShape::Line {
                    length: shape.length,
                },
            },
            track: EmitterTrack::default(),
            emission: Emission {
                rate: emission.rate,
                bursts: bursts(&emission.burst),
                start: emission.timing.start,
                end: emission.timing.end(),
                lifetime: emission.lifetime,
                lifetime_variance: emission.lifetime_variance,
                max_particles: emission.max_particles,
            },
            launch: self.launch,
            forces: Forces {
                gravity: forces.gravity,
                drag: forces.drag,
                drag_variance: forces.drag_variance,
                wind: forces.wind,
                turbulence: forces.turbulence,
                attractor: forces.attracts.then_some(forces.attractor),
            },
            ground: self.ground.enabled.then_some(self.ground.ground),
            appearance: appearance(&self.appearance, sprite),
            lighting: match self.lit {
                true => ParticleLighting::Lit {
                    translucency: self.translucency,
                },
                false => ParticleLighting::Unlit,
            },
            layers: self.layers,
            bloom: self.bloom,
        }
    }
}

fn bursts(burst: &BurstLook) -> Vec<Burst> {
    if burst.count == 0 {
        return Vec::new();
    }
    (0..=burst.repeats)
        .map(|repeat| {
            Burst::new(burst.time + repeat as f32 * burst.interval, burst.count).over(burst.spread)
        })
        .collect()
}

fn over_life(start: f32, end_share: f32) -> Curve {
    match end_share == 1.0 {
        true => Curve::constant(start),
        false => Curve::linear(start, start * end_share),
    }
}

fn appearance(look: &AppearanceLook, sprite: Option<Sprite>) -> Appearance {
    let [birth, early, late, death] = look.colors;
    Appearance {
        shape: look.particle,
        sprite,
        size: over_life(look.size, look.size_end),
        size_variance: look.size_variance,
        aspect: look.aspect,
        color: Gradient::keyed([
            (0.0, birth),
            (look.early, early),
            (look.late, late),
            (1.0, death),
        ]),
        coloring: look.coloring,
        opacity: Curve::keyed([
            (0.0, 0.0),
            (look.fade_in, look.opacity),
            (1.0 - look.fade_out, look.opacity),
            (1.0, 0.0),
        ]),
        emission: over_life(look.emission / NITS_PER_RADIANCE_UNIT, look.emission_end),
        twinkle: look.twinkle,
        rotation: look.rotation,
        spin: look.spin,
        spin_variance: look.spin_variance,
        stretch: look.stretch,
        facing: match look.facing {
            Facing::Camera => ParticleFacing::Camera,
            Facing::Velocity => ParticleFacing::Velocity,
            Facing::Horizontal => ParticleFacing::Horizontal,
            Facing::Axis => ParticleFacing::Axis(look.facing_axis),
            Facing::Tumble => ParticleFacing::Tumble,
        },
        softness: look.softness,
        blend: look.blend,
    }
}
