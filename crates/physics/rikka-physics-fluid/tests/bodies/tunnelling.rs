use glam::{Mat4, Vec3};
use rikka_physics_fluid::{Emission, Inputs, Nozzle};

#[test]
fn fast_thin_sheet_does_not_tunnel() {
    let mut fluid = crate::support::fluid(crate::support::small().radius(0.002));
    fluid
        .set_substances(&[crate::support::honey()])
        .expect("honey");
    let sheet = crate::support::sheet("sheet", 0.05, 10);
    let body = fluid.add_body(&sheet).expect("a sheet");
    let mut height = 0.05;
    for frame in 0..=36u32 {
        let time = frame as f32 / 60.0;
        height = 0.05 + 1.5 * (time - 0.3).max(0.0);
        let mut inputs = Inputs::new().body(
            body,
            &crate::support::placed(&sheet, Mat4::from_translation(Vec3::Y * height)),
        );
        if frame == 0 {
            inputs = inputs.emit(
                Emission::new(0, Nozzle::Blob)
                    .at(Vec3::new(0.0, 0.065, 0.0))
                    .volume(6.0e-6),
            );
        }
        fluid.advance_to(f64::from(time), &inputs).expect("a frame");
    }
    assert!(height > 0.13);
    let particles = fluid.read_particles().expect("particles");
    let inside = |particle: &&rikka_physics_fluid::Particle| {
        particle.position.x.abs() < 0.04 && particle.position.z.abs() < 0.04
    };
    let below: Vec<_> = particles
        .iter()
        .filter(inside)
        .filter(|particle| particle.position.y < height)
        .map(|particle| particle.position)
        .collect();
    assert!(particles.iter().filter(inside).count() > 20);
    assert!(
        below.is_empty(),
        "{below:?} went through the sheet at {height} m"
    );
}
