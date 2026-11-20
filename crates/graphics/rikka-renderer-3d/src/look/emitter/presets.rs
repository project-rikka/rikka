use glam::{Mat4, Vec3};

use super::Emitter;
use super::appearance::{
    Appearance, Coloring, ParticleBlend, ParticleFacing, ParticleLighting, ParticleShape, Stretch,
    Twinkle,
};
use super::curve::{Curve, Gradient};
use super::emission::Emission;
use super::forces::{Forces, Ground, Launch, STANDARD_GRAVITY, Turbulence};
use super::shape::EmitterShape;
use super::track::EmitterTrack;
use crate::look::NITS_PER_RADIANCE_UNIT;

const RAIN_FALL: f32 = 9.0;
const SNOW_FALL: f32 = 1.0;
const PETAL_FALL: f32 = 1.6;
const CONFETTI_FALL: f32 = 2.45;
const LEAF_FALL: f32 = 2.0;
const SPARK_NITS: f32 = 300_000.0;
const EMBER_NITS: f32 = 250_000.0;
const FIREFLY_NITS: f32 = 60_000.0;
const MAGIC_NITS: f32 = 250_000.0;
const RAIN_DENSITY: f32 = 80.0;
const SNOW_DENSITY: f32 = 30.0;
const PETAL_DENSITY: f32 = 6.0;
const CONFETTI_DENSITY: f32 = 10.0;
const LEAF_DENSITY: f32 = 2.0;

fn radiance(nits: f32) -> f32 {
    nits / NITS_PER_RADIANCE_UNIT
}

fn floor_area(area: Vec3) -> f32 {
    4.0 * area.x.abs() * area.z.abs()
}

fn above(area: Vec3) -> EmitterTrack {
    EmitterTrack::fixed(Mat4::from_translation(Vec3::new(0.0, area.y, 0.0)))
}

fn slab(area: Vec3) -> EmitterShape {
    EmitterShape::Box {
        half_extents: Vec3::new(area.x, 0.0, area.z),
    }
}

fn drag_for(fall: f32) -> f32 {
    -STANDARD_GRAVITY.y / fall
}

fn faded(peak: f32) -> Curve {
    Curve::keyed([(0.0, 0.0), (0.08, peak), (0.85, peak), (1.0, 0.0)])
}

fn falling(amount: f32, area: Vec3, wind: Vec3, fall: f32, density: f32, linger: f32) -> Emitter {
    let drag = drag_for(fall);
    Emitter {
        shape: slab(area),
        track: above(area),
        emission: Emission {
            rate: amount.max(0.0) * density * floor_area(area),
            lifetime: area.y.abs() / fall + linger,
            max_particles: 200_000,
            ..Emission::default()
        },
        launch: Launch {
            speed: 0.0,
            velocity: wind + Vec3::new(0.0, -fall, 0.0),
            ..Launch::default()
        },
        forces: Forces {
            gravity: STANDARD_GRAVITY,
            drag,
            wind,
            ..Forces::default()
        },
        ..Emitter::default()
    }
}

impl Emitter {
    pub fn rain(amount: f32, area: Vec3, wind: Vec3) -> Self {
        let rain = falling(amount, area, wind, RAIN_FALL, RAIN_DENSITY, 0.3);
        Self {
            ground: Some(Ground::die_at(0.0)),
            appearance: Appearance {
                shape: ParticleShape::Streak,
                size: Curve::constant(0.012),
                aspect: 0.3,
                color: Gradient::constant(Vec3::new(0.75, 0.8, 0.85)),
                opacity: Curve::keyed([(0.0, 0.0), (0.05, 0.6), (1.0, 0.6)]),
                stretch: Stretch {
                    seconds: 0.04,
                    min: 0.08,
                    max: 0.6,
                },
                facing: ParticleFacing::Velocity,
                softness: 0.05,
                blend: ParticleBlend::Alpha,
                ..Appearance::default()
            },
            lighting: ParticleLighting::Lit { translucency: 0.7 },
            bloom: false,
            ..rain
        }
    }

    pub fn snow(amount: f32, area: Vec3, wind: Vec3) -> Self {
        let mut snow = falling(amount, area, wind, SNOW_FALL, SNOW_DENSITY, 2.5);
        snow.forces.turbulence = Turbulence {
            strength: 0.25,
            frequency: 0.6,
            flow: 0.4,
        };
        Self {
            ground: Some(Ground::settle_at(0.0, 2.0)),
            appearance: Appearance {
                shape: ParticleShape::Flake,
                size: Curve::constant(0.02),
                size_variance: 0.4,
                color: Gradient::constant(Vec3::new(0.95, 0.97, 1.0)),
                opacity: Curve::keyed([(0.0, 0.0), (0.05, 0.9), (1.0, 0.9)]),
                rotation: std::f32::consts::PI,
                spin: 1.0,
                spin_variance: 0.5,
                softness: 0.05,
                ..Appearance::default()
            },
            lighting: ParticleLighting::Lit { translucency: 0.4 },
            bloom: false,
            ..snow
        }
    }

    pub fn sparks(amount: f32, speed: f32, wind: Vec3) -> Self {
        let hot = radiance(SPARK_NITS);
        Self {
            shape: EmitterShape::Cone {
                radius: 0.02,
                angle: 0.6,
            },
            emission: Emission {
                rate: amount.max(0.0) * 300.0,
                lifetime: 1.1,
                lifetime_variance: 0.6,
                ..Emission::default()
            },
            launch: Launch {
                speed,
                speed_variance: 0.35,
                ..Launch::default()
            },
            forces: Forces {
                gravity: STANDARD_GRAVITY,
                drag: 0.4,
                wind,
                ..Forces::default()
            },
            ground: Some(Ground {
                fade: 0.2,
                ..Ground::bounce_at(0.0, 0.35, 0.4)
            }),
            appearance: Appearance {
                shape: ParticleShape::Streak,
                size: Curve::constant(0.006),
                color: Gradient::keyed([
                    (0.0, Vec3::new(1.0, 0.95, 0.8)),
                    (0.3, Vec3::new(1.0, 0.75, 0.3)),
                    (0.7, Vec3::new(1.0, 0.35, 0.08)),
                    (1.0, Vec3::new(0.6, 0.1, 0.02)),
                ]),
                opacity: Curve::keyed([(0.0, 1.0), (0.8, 1.0), (1.0, 0.0)]),
                emission: Curve::keyed([(0.0, hot), (0.5, 0.5 * hot), (1.0, 0.05 * hot)]),
                stretch: Stretch {
                    seconds: 0.035,
                    min: 0.01,
                    max: 0.3,
                },
                facing: ParticleFacing::Velocity,
                softness: 0.02,
                blend: ParticleBlend::Additive,
                ..Appearance::default()
            },
            lighting: ParticleLighting::Unlit,
            ..Self::default()
        }
    }

    pub fn embers(amount: f32, radius: f32, wind: Vec3) -> Self {
        let warm = radiance(EMBER_NITS);
        Self {
            shape: EmitterShape::Disc { radius },
            emission: Emission {
                rate: amount.max(0.0) * 40.0,
                lifetime: 3.0,
                lifetime_variance: 0.5,
                ..Emission::default()
            },
            launch: Launch {
                speed: 0.3,
                speed_variance: 0.5,
                velocity: Vec3::new(0.0, 0.6, 0.0),
                inherit: 0.0,
            },
            forces: Forces {
                gravity: Vec3::new(0.0, 0.35, 0.0),
                drag: 0.9,
                wind,
                turbulence: Turbulence {
                    strength: 0.5,
                    frequency: 1.2,
                    flow: 0.6,
                },
                ..Forces::default()
            },
            appearance: Appearance {
                shape: ParticleShape::Soft,
                size: Curve::constant(0.02),
                size_variance: 0.4,
                color: Gradient::linear(Vec3::new(1.0, 0.55, 0.15), Vec3::new(0.8, 0.2, 0.03)),
                opacity: faded(1.0),
                emission: Curve::linear(warm, 0.2 * warm),
                twinkle: Twinkle {
                    depth: 0.5,
                    rate: 2.5,
                },
                softness: 0.05,
                blend: ParticleBlend::Additive,
                ..Appearance::default()
            },
            lighting: ParticleLighting::Unlit,
            ..Self::default()
        }
    }

    pub fn petals(amount: f32, area: Vec3, wind: Vec3) -> Self {
        let mut petals = falling(amount, area, wind, PETAL_FALL, PETAL_DENSITY, 4.0);
        petals.forces.drag_variance = 0.3;
        petals.forces.turbulence = Turbulence {
            strength: 0.35,
            frequency: 0.5,
            flow: 0.5,
        };
        Self {
            ground: Some(Ground::settle_at(0.0, 3.0)),
            appearance: tumbling(
                ParticleShape::Petal,
                Curve::constant(0.035),
                0.75,
                Gradient::linear(Vec3::new(1.0, 0.78, 0.84), Vec3::new(0.98, 0.62, 0.74)),
                2.5,
            ),
            lighting: ParticleLighting::Lit { translucency: 0.6 },
            bloom: false,
            ..petals
        }
    }

    pub fn fireflies(amount: f32, area: Vec3, wind: Vec3) -> Self {
        Self {
            shape: EmitterShape::Box { half_extents: area },
            track: above(area),
            emission: Emission {
                rate: amount.max(0.0) * 25.0,
                lifetime: 8.0,
                lifetime_variance: 0.3,
                ..Emission::default()
            },
            launch: Launch {
                speed: 0.05,
                ..Launch::default()
            },
            forces: Forces {
                drag: 1.5,
                wind,
                turbulence: Turbulence {
                    strength: 0.35,
                    frequency: 0.4,
                    flow: 0.25,
                },
                ..Forces::default()
            },
            appearance: Appearance {
                shape: ParticleShape::Soft,
                size: Curve::constant(0.03),
                color: Gradient::constant(Vec3::new(0.75, 1.0, 0.35)),
                opacity: faded(1.0),
                emission: Curve::constant(radiance(FIREFLY_NITS)),
                twinkle: Twinkle {
                    depth: 1.0,
                    rate: 0.35,
                },
                softness: 0.05,
                blend: ParticleBlend::Additive,
                ..Appearance::default()
            },
            lighting: ParticleLighting::Unlit,
            ..Self::default()
        }
    }

    pub fn dust(amount: f32, radius: f32, wind: Vec3) -> Self {
        Self {
            shape: EmitterShape::Cone { radius, angle: 1.2 },
            emission: Emission {
                rate: amount.max(0.0) * 30.0,
                lifetime: 2.5,
                lifetime_variance: 0.4,
                ..Emission::default()
            },
            launch: Launch {
                speed: 0.8,
                speed_variance: 0.5,
                ..Launch::default()
            },
            forces: Forces {
                gravity: Vec3::new(0.0, -0.3, 0.0),
                drag: 2.0,
                wind,
                turbulence: Turbulence {
                    strength: 0.25,
                    frequency: 1.0,
                    flow: 0.5,
                },
                ..Forces::default()
            },
            appearance: Appearance {
                shape: ParticleShape::Soft,
                size: Curve::linear(0.12, 0.7),
                color: Gradient::linear(Vec3::new(0.72, 0.62, 0.5), Vec3::new(0.8, 0.74, 0.66)),
                opacity: Curve::keyed([(0.0, 0.0), (0.15, 0.35), (1.0, 0.0)]),
                rotation: std::f32::consts::PI,
                spin: 0.4,
                spin_variance: 0.5,
                softness: 0.3,
                ..Appearance::default()
            },
            lighting: ParticleLighting::Lit { translucency: 0.5 },
            bloom: false,
            ..Self::default()
        }
    }

    pub fn magic(amount: f32, radius: f32, wind: Vec3) -> Self {
        let bright = radiance(MAGIC_NITS);
        Self {
            shape: EmitterShape::Sphere { radius },
            emission: Emission {
                rate: amount.max(0.0) * 60.0,
                lifetime: 2.4,
                lifetime_variance: 0.4,
                ..Emission::default()
            },
            launch: Launch {
                speed: 0.15,
                speed_variance: 0.5,
                velocity: Vec3::new(0.0, 0.5, 0.0),
                inherit: 0.0,
            },
            forces: Forces {
                drag: 0.6,
                wind,
                turbulence: Turbulence {
                    strength: 0.3,
                    frequency: 1.5,
                    flow: 0.8,
                },
                ..Forces::default()
            },
            appearance: Appearance {
                shape: ParticleShape::Star,
                size: Curve::keyed([(0.0, 0.03), (0.3, 0.05), (1.0, 0.015)]),
                size_variance: 0.4,
                color: Gradient::keyed([
                    (0.0, Vec3::new(0.6, 0.9, 1.0)),
                    (0.5, Vec3::new(0.75, 0.55, 1.0)),
                    (1.0, Vec3::new(1.0, 0.6, 0.95)),
                ]),
                opacity: faded(1.0),
                emission: Curve::linear(bright, 0.3 * bright),
                twinkle: Twinkle {
                    depth: 0.8,
                    rate: 4.0,
                },
                rotation: std::f32::consts::PI,
                spin: 1.5,
                softness: 0.05,
                blend: ParticleBlend::Additive,
                ..Appearance::default()
            },
            lighting: ParticleLighting::Unlit,
            ..Self::default()
        }
    }

    pub fn confetti(amount: f32, area: Vec3, wind: Vec3) -> Self {
        let mut confetti = falling(amount, area, wind, CONFETTI_FALL, CONFETTI_DENSITY, 5.0);
        confetti.forces.drag_variance = 0.3;
        confetti.forces.turbulence = Turbulence {
            strength: 0.2,
            frequency: 0.8,
            flow: 0.5,
        };
        Self {
            ground: Some(Ground::settle_at(0.0, 4.0)),
            appearance: tumbling(
                ParticleShape::Square,
                Curve::constant(0.035),
                0.55,
                Gradient::keyed([
                    (0.0, Vec3::new(0.95, 0.15, 0.2)),
                    (0.33, Vec3::new(1.0, 0.85, 0.1)),
                    (0.66, Vec3::new(0.1, 0.7, 0.95)),
                    (1.0, Vec3::new(0.85, 0.2, 0.85)),
                ]),
                7.0,
            ),
            lighting: ParticleLighting::Lit { translucency: 0.35 },
            bloom: false,
            ..confetti
        }
    }

    pub fn leaves(amount: f32, area: Vec3, wind: Vec3) -> Self {
        let mut leaves = falling(amount, area, wind, LEAF_FALL, LEAF_DENSITY, 6.0);
        leaves.forces.drag_variance = 0.35;
        leaves.forces.turbulence = Turbulence {
            strength: 0.45,
            frequency: 0.35,
            flow: 0.4,
        };
        Self {
            ground: Some(Ground::settle_at(0.0, 5.0)),
            appearance: tumbling(
                ParticleShape::Leaf,
                Curve::constant(0.06),
                0.6,
                Gradient::keyed([
                    (0.0, Vec3::new(0.85, 0.45, 0.08)),
                    (0.33, Vec3::new(0.75, 0.2, 0.05)),
                    (0.66, Vec3::new(0.9, 0.7, 0.15)),
                    (1.0, Vec3::new(0.45, 0.35, 0.1)),
                ]),
                2.0,
            ),
            lighting: ParticleLighting::Lit { translucency: 0.5 },
            bloom: false,
            ..leaves
        }
    }
}

fn tumbling(
    shape: ParticleShape,
    size: Curve,
    aspect: f32,
    color: Gradient,
    spin: f32,
) -> Appearance {
    Appearance {
        shape,
        size,
        size_variance: 0.25,
        aspect,
        color,
        coloring: Coloring::PerParticle,
        opacity: Curve::keyed([(0.0, 0.0), (0.03, 1.0), (1.0, 1.0)]),
        rotation: std::f32::consts::PI,
        spin,
        spin_variance: 0.5,
        facing: ParticleFacing::Tumble,
        softness: 0.03,
        ..Appearance::default()
    }
}
