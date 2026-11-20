use glam::{Mat4, Vec3};
use rikka_physics_fluid::{Emission, Fluid, Inputs, Nozzle, Substance};

const PARTICLE_LANES: [usize; 13] = [0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14];
const SPRAY_LANES: [usize; 7] = [0, 1, 2, 3, 4, 5, 6];

fn liquid(
    viscosity: f32,
    elasticity: f32,
    relaxation: f32,
    cohesion: f32,
    (adhesion, friction, spray, wetting): (f32, f32, f32, f32),
) -> Substance {
    Substance {
        viscosity,
        elasticity,
        relaxation,
        cohesion,
        adhesion,
        friction,
        spray,
        wetting,
        tint: [0.8, 0.8, 0.8],
    }
}

fn presets() -> [(&'static str, Substance); 6] {
    [
        ("water", liquid(0.0, 0.0, 1.0, 0.4, (0.05, 0.5, 0.5, 0.5))),
        ("slime", liquid(0.55, 0.55, 0.15, 0.8, (0.8, 0.8, 0.0, 0.3))),
        ("lotion", liquid(0.6, 0.45, 0.7, 0.7, (0.85, 0.7, 0.0, 0.8))),
        ("honey", liquid(0.78, 0.0, 1.0, 0.5, (0.6, 0.6, 0.0, 0.6))),
        ("milk", liquid(0.2, 0.0, 1.0, 0.4, (0.15, 0.5, 0.4, 0.7))),
        ("blood", liquid(0.35, 0.05, 0.9, 0.35, (0.4, 0.3, 0.2, 1.0))),
    ]
}

fn check(name: &str, frame: u32, what: &str, values: impl IntoIterator<Item = f32>) {
    if let Some(bad) = values.into_iter().find(|value| !value.is_finite()) {
        panic!("{name} produced a non-finite {what} ({bad}) in frame {frame}");
    }
}

fn lanes(bytes: &[u8], stride: usize, lanes_used: &[usize]) -> Vec<f32> {
    crate::support::floats(bytes)
        .chunks_exact(stride)
        .flat_map(|record| {
            lanes_used
                .iter()
                .map(|&lane| record[lane])
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn presets_stay_finite() {
    for (name, substance) in presets() {
        let mut fluid = crate::support::fluid(
            Fluid::builder()
                .radius(0.004)
                .capacity(1 << 13)
                .spray_capacity(2048)
                .substeps(2, 16)
                .floor(0.0),
        );
        fluid.set_substances(&[substance]).expect("a preset");
        let sheet = crate::support::sheet("sheet", 0.08, 4);
        let body = fluid.add_body(&sheet).expect("a sheet");
        let mut peak_particles = 0usize;
        for frame in 0..=40u32 {
            let time = frame as f32 / 60.0;
            let placement = Mat4::from_translation(Vec3::new(0.0, 0.1 + 0.05 * time, 0.0))
                * Mat4::from_rotation_z(0.4 * (6.0 * time).sin());
            let mut inputs = Inputs::new().body(body, &crate::support::placed(&sheet, placement));
            if frame == 0 {
                inputs = inputs.emit(
                    Emission::new(0, Nozzle::Blob)
                        .at(Vec3::new(0.03, 0.25, 0.0))
                        .volume(2.0e-5),
                );
            }
            if frame < 30 {
                inputs = inputs.emit(
                    Emission::new(1, Nozzle::Stream { radius: 0.008 })
                        .at(Vec3::new(-0.03, 0.35, 0.0))
                        .velocity(Vec3::new(0.5, -2.0, 0.0))
                        .rate(4.0e-4),
                );
            }
            fluid.advance_to(f64::from(time), &inputs).expect("a frame");
            if frame % 5 != 0 {
                continue;
            }
            let particles = fluid.read_particles().expect("particles");
            peak_particles = peak_particles.max(particles.len());
            check(
                name,
                frame,
                "particle",
                particles.iter().flat_map(|particle| {
                    particle
                        .position
                        .to_array()
                        .into_iter()
                        .chain(particle.velocity.to_array())
                }),
            );
            let (shown, spray) = crate::support::surface_bytes(&fluid.surface());
            check(
                name,
                frame,
                "surface record",
                lanes(&shown, 16, &PARTICLE_LANES),
            );
            check(name, frame, "spray record", lanes(&spray, 8, &SPRAY_LANES));
            let wetness = fluid.read_wetness(body).expect("wetness");
            check(name, frame, "wetness", wetness.into_iter().flatten());
        }
        assert!(peak_particles > 0, "{name} never emitted");
    }
}
