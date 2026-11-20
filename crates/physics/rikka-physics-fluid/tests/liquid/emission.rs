use glam::Vec3;
use rikka_physics_fluid::{Emission, Inputs, Nozzle};

#[test]
fn emitted_volume_matches_rate() {
    let mut fluid = crate::support::fluid(crate::support::small());
    let rate = 4.0e-5;
    let seconds = 30u32;
    for frame in 0..=seconds {
        let inputs = Inputs::new()
            .emit(
                Emission::new(3, Nozzle::Stream { radius: 0.006 })
                    .at(Vec3::new(0.0, 0.5, 0.0))
                    .velocity(Vec3::new(0.0, -0.5, 0.0))
                    .rate(rate),
            )
            .emit(
                Emission::new(
                    4,
                    Nozzle::Shower {
                        radius: 0.01,
                        spread: 0.3,
                    },
                )
                .at(Vec3::new(0.2, 0.5, 0.0))
                .velocity(Vec3::new(0.0, -1.0, 0.0))
                .rate(rate),
            );
        fluid
            .advance_to(f64::from(frame) / 60.0, &inputs)
            .expect("a frame");
    }
    let particles = fluid.read_particles().expect("particles");
    let expected = 2.0 * f64::from(rate) * f64::from(seconds) / 60.0;
    let emitted = particles.len() as f64 * f64::from(fluid.particle_volume());
    let error = (emitted - expected).abs() / expected;
    assert!(error < 0.05, "emitted {emitted} m³ against {expected} m³");
}

#[test]
fn blob_preserves_volume() {
    let mut fluid = crate::support::fluid(crate::support::small());
    let volume = 3.0e-5f32;
    let blob = Emission::new(5, Nozzle::Blob).at(Vec3::Y).volume(volume);
    fluid
        .advance_to(0.0, &Inputs::new().emit(blob))
        .expect("a blob");
    let count = fluid.read_particles().expect("particles").len();
    let emitted = count as f32 * fluid.particle_volume();
    assert!(
        (emitted - volume).abs() / volume < 0.1,
        "the blob holds {emitted} m³"
    );
}

#[test]
fn emission_stops_at_capacity() {
    let mut fluid = crate::support::fluid(crate::support::small().capacity(300));
    let blob = Emission::new(6, Nozzle::Blob).at(Vec3::Y).volume(1.0e-4);
    fluid
        .advance_to(0.0, &Inputs::new().emit(blob.clone()))
        .expect("a blob");
    fluid
        .advance_to(0.1, &Inputs::new().emit(blob))
        .expect("another blob");
    assert_eq!(fluid.read_particles().expect("particles").len(), 300);
}
