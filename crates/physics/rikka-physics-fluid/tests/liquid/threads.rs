use glam::{Mat4, Vec3};
use rikka_physics_fluid::{Emission, Inputs, Nozzle, Particle};

fn parting(time: f32) -> f32 {
    match time {
        t if t < 0.3 => 0.022 - 0.01 * t / 0.3,
        t if t < 0.4 => 0.012,
        t => 0.012 + 0.06 * (t - 0.4),
    }
}

fn spans(particles: &[Particle], gap: f32, reach: f32) -> bool {
    crate::support::connected(
        particles,
        reach,
        |particle| particle.position.y < 0.005,
        |particle| particle.position.y > gap - 0.005,
    )
}

#[test]
fn viscous_thread_stretches_then_breaks() {
    let mut fluid = crate::support::fluid(crate::support::small().radius(0.002));
    fluid
        .set_substances(&[crate::support::lotion()])
        .expect("lotion");
    let plate = crate::support::sheet("plate", 0.03, 10);
    let lower = fluid.add_body(&plate).expect("a lower plate");
    let upper = fluid.add_body(&plate).expect("an upper plate");
    let reach = 2.0 * 2.0 * fluid.radius();
    let mut stretched = None;
    for frame in 0..=80u32 {
        let time = frame as f32 / 60.0;
        let gap = parting(time);
        let mut inputs = Inputs::new()
            .body(lower, &crate::support::placed(&plate, Mat4::IDENTITY))
            .body(
                upper,
                &crate::support::placed(&plate, Mat4::from_translation(Vec3::Y * gap)),
            );
        if frame == 0 {
            inputs = inputs.emit(
                Emission::new(0, Nozzle::Blob)
                    .at(Vec3::new(0.0, 0.011, 0.0))
                    .volume(4.5e-6),
            );
        }
        fluid.advance_to(f64::from(time), &inputs).expect("a frame");
        if frame == 40 {
            stretched = Some((gap, fluid.read_particles().expect("particles")));
        }
    }
    let (gap, particles) = stretched.expect("the stretched frame");
    assert!(gap > 0.027);
    assert!(
        spans(&particles, gap, reach),
        "no thread spans the {gap} m gap"
    );
    let gap = parting(80.0 / 60.0);
    assert!(
        !spans(&fluid.read_particles().expect("particles"), gap, reach),
        "the thread still spans the {gap} m gap"
    );
}
