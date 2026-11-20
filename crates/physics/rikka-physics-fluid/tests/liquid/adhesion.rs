use glam::{Mat4, Vec3};
use rikka_physics_fluid::{Emission, Inputs, Nozzle, Substance};

fn held_under_plate(viscosity: f32, adhesion: f32, cohesion: f32) -> f32 {
    let mut fluid = crate::support::fluid(crate::support::small());
    fluid
        .set_substances(&[Substance {
            viscosity,
            adhesion,
            cohesion,
            ..Substance::default()
        }])
        .expect("a substance");
    let plate = crate::support::sheet("overhang", 0.04, 8);
    let body = fluid.add_body(&plate).expect("a plate");
    let pose = crate::support::placed(&plate, Mat4::from_translation(Vec3::Y * 0.1));
    for frame in 0..=40u32 {
        let mut inputs = Inputs::new().body(body, &pose);
        if frame == 0 {
            inputs = inputs.emit(
                Emission::new(0, Nozzle::Blob)
                    .at(Vec3::new(0.0, 0.093, 0.0))
                    .volume(4.0e-6),
            );
        }
        fluid
            .advance_to(f64::from(frame) / 60.0, &inputs)
            .expect("a frame");
    }
    let particles = fluid.read_particles().expect("particles");
    let stuck = particles
        .iter()
        .filter(|particle| particle.position.y > 0.075 && particle.position.y < 0.1)
        .count();
    stuck as f32 / particles.len().max(1) as f32
}

#[test]
fn adhesion_holds_liquid_under_overhang() {
    let sticky = held_under_plate(0.8, 1.0, 0.8);
    let slick = held_under_plate(0.0, 0.0, 0.0);
    assert!(sticky > 0.7, "only {sticky} of the sticky liquid stayed");
    assert!(slick < 0.3, "{slick} of the slick liquid stayed");
}
