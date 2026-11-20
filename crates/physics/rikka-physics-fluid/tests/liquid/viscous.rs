use glam::Vec3;
use rikka_physics_fluid::{Emission, Fluid, Inputs, Nozzle, Particle, Substance};

const HEIGHT: f32 = 1.0;
const SPEED: f32 = 5.0;
const JET: f32 = 0.008;
const GRAVITY: f32 = 9.81;
const SPEED_SLACK: f32 = 0.05;
const OFF_PATH: f32 = 0.03;
const STRAYS: f32 = 0.01;

fn thick(viscosity: f32) -> Substance {
    Substance {
        viscosity,
        cohesion: 0.4,
        adhesion: 0.15,
        spray: 0.4,
        ..Substance::default()
    }
}

fn liquid(substance: Substance) -> Fluid {
    let mut fluid = crate::support::fluid(
        Fluid::builder()
            .radius(0.004)
            .capacity(1 << 13)
            .substeps(2, 16)
            .floor(0.0),
    );
    fluid.set_substances(&[substance]).expect("a liquid");
    fluid
}

fn fastest_allowed(particle: &Particle) -> f32 {
    (SPEED * SPEED + 2.0 * GRAVITY * (HEIGHT - particle.position.y).max(0.0)).sqrt()
        + SPEED_SLACK * SPEED
}

fn off_path(particle: &Particle) -> bool {
    let flown = particle.position.x / SPEED;
    let path = HEIGHT - 0.5 * GRAVITY * flown * flown;
    (particle.position.y - path).abs() > OFF_PATH || particle.position.z.abs() > OFF_PATH
}

fn stray_share(name: &str, substance: Substance) -> f32 {
    let mut fluid = liquid(substance);
    let mut worst = 0.0f32;
    for frame in 0..=24u32 {
        let stream = Emission::new(0, Nozzle::Stream { radius: JET })
            .at(Vec3::new(0.0, HEIGHT, 0.0))
            .velocity(Vec3::X * SPEED)
            .rate(std::f32::consts::PI * JET * JET * SPEED);
        fluid
            .advance_to(f64::from(frame) / 60.0, &Inputs::new().emit(stream))
            .expect("a frame");
        let particles = fluid.read_particles().expect("particles");
        for particle in &particles {
            assert!(
                particle.velocity.length() <= fastest_allowed(particle),
                "a {name} stream particle at {:?} flies at {} m/s",
                particle.position,
                particle.velocity.length()
            );
        }
        let strays = particles
            .iter()
            .filter(|particle| off_path(particle))
            .count();
        worst = worst.max(strays as f32 / particles.len().max(1) as f32);
    }
    worst
}

#[test]
fn viscous_stream_stays_coherent() {
    for viscosity in [0.0, 0.2, 0.5] {
        let strays = stray_share(&format!("viscosity {viscosity}"), thick(viscosity));
        assert!(
            strays < STRAYS,
            "{:.1}% of a viscosity {viscosity} stream left its path",
            strays * 100.0
        );
    }
}
