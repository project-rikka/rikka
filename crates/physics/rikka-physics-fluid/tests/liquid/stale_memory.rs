use glam::Vec3;
use rikka_gpu::ComputeBuffer;
use rikka_physics_fluid::{Emission, Inputs, Nozzle};

fn poured() -> Vec<u32> {
    let mut fluid = crate::support::fluid(crate::support::small().floor(0.0));
    fluid
        .set_substances(&[crate::support::water(), crate::support::honey()])
        .expect("substances");
    for frame in 0..12u32 {
        let mut inputs = Inputs::new();
        if frame < 8 {
            inputs = inputs.emit(
                Emission::new(1, Nozzle::Stream { radius: 0.006 })
                    .at(Vec3::new(0.0, 0.1, 0.0))
                    .velocity(Vec3::new(0.0, -0.5, 0.0))
                    .rate(4.0e-5)
                    .substance(1),
            );
        }
        fluid
            .advance_to(f64::from(frame) / 60.0, &inputs)
            .expect("a frame");
    }
    fluid
        .read_particles()
        .expect("particles")
        .iter()
        .flat_map(|particle| {
            particle
                .position
                .to_array()
                .into_iter()
                .chain(particle.velocity.to_array())
        })
        .map(f32::to_bits)
        .collect()
}

fn littered_with_nan() -> Vec<ComputeBuffer> {
    let compute = crate::support::compute();
    let mut freed = Vec::new();
    let mut reserved = Vec::new();
    for at in 0..600u64 {
        freed.push(compute.buffer((1 + at % 7) << 16).expect("a buffer"));
        reserved.push(compute.buffer(256).expect("a buffer reserved apart"));
    }
    let mut batch = compute.batch();
    for buffer in &freed {
        batch.fill(buffer, 0x7fc0_0001);
    }
    batch
        .submit()
        .expect("the litter")
        .wait()
        .expect("littered");
    drop(freed);
    compute.wait_idle().expect("an idle device");
    reserved
}

#[test]
fn stale_nan_memory_does_not_affect_viscous_pour() {
    let clean = poured();
    assert!(!clean.is_empty());
    let _kept = littered_with_nan();
    assert!(poured() == clean, "the pour read memory it never wrote");
}
