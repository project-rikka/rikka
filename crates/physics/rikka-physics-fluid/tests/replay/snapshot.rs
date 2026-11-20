use glam::Vec3;
use rikka_physics_fluid::{Emission, Fluid, Inputs, Nozzle};

fn inputs(frame: u32) -> Inputs {
    let mut inputs = Inputs::new();
    if frame == 0 {
        inputs = inputs.emit(
            Emission::new(0, Nozzle::Blob)
                .at(Vec3::new(0.0, 0.05, 0.0))
                .volume(6.0e-6),
        );
    }
    if frame < 10 {
        inputs = inputs.emit(
            Emission::new(1, Nozzle::Stream { radius: 0.005 })
                .at(Vec3::new(0.02, 0.1, 0.0))
                .velocity(Vec3::new(-0.3, -0.3, 0.0))
                .rate(2.0e-5),
        );
    }
    inputs
}

fn advance(fluid: &mut Fluid, frame: u32) {
    fluid
        .advance_to(f64::from(frame) / 60.0, &inputs(frame))
        .expect("a frame");
}

#[test]
fn restored_checkpoint_matches_live_run() {
    let mut fluid = crate::support::fluid(crate::support::small().floor(0.0));
    fluid
        .set_substances(&[crate::support::water(), crate::support::slime()])
        .expect("substances");
    let mut live = Vec::new();
    let mut captures = Vec::new();
    let mut checkpoint = None;
    for frame in 0..=12 {
        advance(&mut fluid, frame);
        live.push(crate::support::surface_bytes(&fluid.surface()));
        captures.push(fluid.capture().expect("a capture"));
        if frame == 4 {
            checkpoint = Some(fluid.checkpoint().expect("a checkpoint"));
        }
    }
    let captures: Vec<_> = captures
        .into_iter()
        .map(|pending_capture| pending_capture.finish().expect("a finished capture"))
        .collect();
    assert!(captures.iter().all(|capture| capture.bytes() > 0));

    fluid.show(&captures[5]).expect("a shown frame");
    assert!(
        crate::support::surface_bytes(&fluid.surface()) == live[5],
        "frame 5 shows differently"
    );

    let checkpoint = checkpoint.expect("frame 4 was kept");
    assert!(checkpoint.bytes() > 0);
    fluid.restore(&checkpoint).expect("a restored frame");
    for frame in 5..=11 {
        advance(&mut fluid, frame);
        assert!(
            crate::support::surface_bytes(&fluid.surface()) == live[frame as usize],
            "frame {frame} runs differently after restoring"
        );
    }
}
