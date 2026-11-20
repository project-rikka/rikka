use glam::{Mat4, Vec3};
use rikka_physics_fluid::{Emission, Inputs, Nozzle};

#[test]
fn settled_column_keeps_rest_density() {
    let mut fluid = crate::support::fluid(crate::support::small().floor(0.0).iterations(4));
    let half = 0.03;
    let cup = crate::support::cup(half, 0.08, 8);
    let body = fluid.add_body(&cup).expect("a cup");
    let pose = crate::support::placed(&cup, Mat4::IDENTITY);
    for frame in 0..=90u32 {
        let mut inputs = Inputs::new().body(body, &pose);
        if frame == 0 {
            inputs = inputs.emit(
                Emission::new(0, Nozzle::Blob)
                    .at(Vec3::new(0.0, 0.04, 0.0))
                    .volume(1.0e-4),
            );
        }
        fluid
            .advance_to(f64::from(frame) / 60.0, &inputs)
            .expect("a frame");
    }
    let particles = fluid.read_particles().expect("particles");
    assert!(particles.iter().all(|particle| {
        particle.position.x.abs() < half + 0.001
            && particle.position.z.abs() < half + 0.001
            && particle.position.y > 0.0
    }));
    let reach = 1.7 * 2.0 * fluid.radius();
    let densities =
        crate::support::relative_densities(&particles, fluid.radius(), 16, |particle| {
            particle.position.x.abs() < half - reach
                && particle.position.z.abs() < half - reach
                && particle.position.y > reach
        });
    assert!(
        densities.len() > 30,
        "{} interior particles",
        densities.len()
    );
    let mean = densities.iter().sum::<f32>() / densities.len() as f32;
    let most = densities.iter().copied().fold(0.0, f32::max);
    assert!((0.93..1.06).contains(&mean), "mean density {mean}");
    assert!(most < 1.2, "densest particle {most}");
    let fastest = particles
        .iter()
        .map(|particle| particle.velocity.length())
        .fold(0.0, f32::max);
    assert!(fastest < 0.15, "the column still moves at {fastest} m/s");
}
