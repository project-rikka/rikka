use std::sync::Arc;

use glam::{Mat4, Vec3};
use rikka_model_core::Instance;
use rikka_physics::{LiveCloth, LiveInputs, Pace, Step};
use rikka_physics_cloth::{Cloth, Inputs};

const FRAME: f32 = 1.0 / 60.0;

#[test]
fn live_cloth_seeks_and_steps_at_fixed_rate() {
    let compute = crate::support::compute();
    let curtain = crate::support::curtain(6, 6, 0.04, 0.6);
    let instance = Instance::new(Arc::clone(&curtain.model));
    let mut live = LiveCloth::new(&compute, Cloth::builder()).expect("a live cloth");
    live.add_figure(0, Arc::clone(&curtain.asset))
        .expect("curtain has cloth");
    assert!(live.has_figure(0) && !live.has_figure(1));
    let follow = |live: &mut LiveCloth, time: f32, pace: Pace| {
        live.follow(time, pace, &LiveInputs::new().figure(0, &instance))
            .expect("a followed frame")
    };

    assert_eq!(follow(&mut live, 2.0, Pace::Playing), Step::Seek);
    assert_eq!(live.steps(), 1, "a seek warms up in one step");

    let step = follow(&mut live, 2.0 + 2.5 * FRAME, Pace::Playing);
    assert!(matches!(step, Step::Advance(_)));
    assert_eq!(live.steps(), 3, "two and a half frames make two steps");
    follow(&mut live, 2.0 + 3.0 * FRAME, Pace::Playing);
    assert_eq!(live.steps(), 4, "the half step left over is owed");

    let idle = Pace::Stopped { idle: FRAME };
    assert_eq!(
        follow(&mut live, 2.0 + 3.0 * FRAME, idle),
        Step::Advance(FRAME)
    );
    assert_eq!(live.steps(), 5, "a stopped clock keeps settling");

    follow(&mut live, 2.9, Pace::Playing);
    assert_eq!(live.steps(), 9, "a long frame runs at most four steps");
    follow(&mut live, 2.9 + FRAME, Pace::Playing);
    assert_eq!(live.steps(), 10, "the dropped time is not owed");

    assert_eq!(follow(&mut live, 0.5, Pace::Playing), Step::Seek);
    assert_eq!(live.steps(), 1, "going back starts over");

    live.restart();
    assert_eq!(follow(&mut live, 0.5 + FRAME, Pace::Playing), Step::Seek);
    assert!(live.motion().is_finite());

    live.add_figure(0, Arc::clone(&curtain.asset))
        .expect("curtain has cloth again");
    live.remove_figure(0).expect("curtain cloth removed");
    assert!(live.is_empty());
}

#[test]
fn batched_steps_match_single_steps() {
    let compute = crate::support::compute();
    let curtain = crate::support::curtain(6, 6, 0.04, 0.6);
    let mut instance = Instance::new(Arc::clone(&curtain.model));
    let mut alone = Cloth::builder().build(&compute).expect("a cloth");
    let mut together = Cloth::builder().build(&compute).expect("a cloth");
    let figure = alone
        .add_figure(Arc::clone(&curtain.asset))
        .expect("curtain has cloth");
    together
        .add_figure(Arc::clone(&curtain.asset))
        .expect("curtain has cloth");
    let per_batch = rikka_physics_cloth::READBACK_LAG as usize;
    let interval = f64::from(alone.rate()).recip();
    for pair in 0..40 {
        let times: Vec<f64> = (0..per_batch)
            .map(|at| (pair * per_batch + at) as f64 * interval)
            .collect();
        let mut batch = compute.batch();
        let mut recorded = Vec::new();
        for &time in &times {
            let sway = if pair < 20 {
                0.1 * (6.0 * time).sin() as f32
            } else {
                0.0
            };
            instance.set_transform(Mat4::from_translation(Vec3::new(sway, 0.0, 0.0)));
            instance.refresh();
            let inputs = Inputs::new().figure(figure, &instance);
            alone.advance(time, &inputs).expect("a step");
            recorded.push(
                together
                    .advance_into(&mut batch, time, &inputs)
                    .expect("a recorded step"),
            );
        }
        let ready = batch.submit().expect("the steps are sent");
        for stepped in recorded {
            together.commit(stepped, &ready);
        }
        assert_eq!(
            crate::support::drapes(&compute, &alone.drapes(figure).expect("drapes")),
            crate::support::drapes(&compute, &together.drapes(figure).expect("drapes")),
            "the drapes after pair {pair}"
        );
        assert_eq!(alone.motion().to_bits(), together.motion().to_bits());
    }
    let particles = |cloth: &mut Cloth| -> Vec<[u32; 6]> {
        cloth
            .read_particles(figure)
            .expect("the particles are read")
            .iter()
            .map(|particle| {
                let [x, y, z] = particle.position.to_array().map(f32::to_bits);
                let [u, v, w] = particle.velocity.to_array().map(f32::to_bits);
                [x, y, z, u, v, w]
            })
            .collect()
    };
    assert_eq!(particles(&mut alone), particles(&mut together));
}
