use glam::{Mat4, Vec3};
use rikka_physics_fluid::{Emission, Inputs, Nozzle};

fn run(reading: bool) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut fluid = crate::support::fluid(crate::support::small().floor(0.0));
    fluid
        .set_substances(&[crate::support::water(), crate::support::slime()])
        .expect("substances");
    let sheet = crate::support::sheet("sheet", 0.06, 8);
    let body = fluid.add_body(&sheet).expect("a body");
    for frame in 0..=24u32 {
        let time = frame as f32 / 60.0;
        let placement = Mat4::from_translation(Vec3::new(0.0, 0.05 + 0.1 * time, 0.0))
            * Mat4::from_rotation_z(0.3);
        let mut inputs = Inputs::new()
            .body(body, &crate::support::placed(&sheet, placement))
            .drying(0.5);
        if frame == 0 {
            inputs = inputs.emit(
                Emission::new(1, Nozzle::Blob)
                    .at(Vec3::new(0.0, 0.12, 0.0))
                    .volume(8.0e-6)
                    .substance(1),
            );
        }
        if frame < 18 {
            inputs = inputs.emit(
                Emission::new(2, Nozzle::Stream { radius: 0.006 })
                    .at(Vec3::new(0.03, 0.2, 0.0))
                    .velocity(Vec3::new(0.0, -0.8, 0.0))
                    .rate(3.0e-5),
            );
        }
        fluid.advance_to(f64::from(time), &inputs).expect("a frame");
        if reading {
            fluid.counts().expect("counts");
            fluid.read_particles().expect("particles");
        }
    }
    let (particles, spray) = crate::support::surface_bytes(&fluid.surface());
    let wetting = fluid.wetness(body).expect("wetness");
    wetting.ready.wait().expect("wetness is made");
    let wet = crate::support::read(&wetting.buffer, wetting.buffer.size());
    let state = fluid
        .read_particles()
        .expect("particles")
        .iter()
        .flat_map(|particle| {
            particle
                .position
                .to_array()
                .into_iter()
                .chain(particle.velocity.to_array())
                .flat_map(f32::to_le_bytes)
        })
        .collect();
    (particles, spray, wet, state)
}

#[test]
fn runs_are_bit_exact() {
    let first = run(false);
    let second = run(false);
    assert!(!first.0.is_empty());
    assert!(
        first.2.iter().any(|&byte| byte != 0),
        "the sheet never got wet"
    );
    assert!(first.0 == second.0, "the particle surfaces differ");
    assert!(first.1 == second.1, "the spray differs");
    assert!(first.2 == second.2, "the wetness differs");
    assert!(first.3 == second.3, "the particle state differs");
}

#[test]
fn readback_does_not_change_result() {
    assert!(
        run(false) == run(true),
        "waiting for every frame changed the simulation"
    );
}
