use glam::{Mat4, Vec3};
use rikka_gpu::ComputeBuffer;
use rikka_model_core::NodeId;
use rikka_physics_fluid::{Emission, Error, Inputs, Nozzle};
use rikka_renderer_3d::DRAPE_VERTEX_BYTES;

#[test]
fn removing_body_keeps_others_working() {
    let mut fluid = crate::support::fluid(crate::support::small());
    fluid
        .set_substances(&[crate::support::honey()])
        .expect("honey");
    let sheet = crate::support::sheet("sheet", 0.04, 6);
    let first = fluid.add_body(&sheet).expect("a first body");
    let second = fluid.add_body(&sheet).expect("a second body");
    let low = crate::support::placed(&sheet, Mat4::from_translation(Vec3::Y * 0.05));
    let high = crate::support::placed(&sheet, Mat4::from_translation(Vec3::new(0.2, 0.05, 0.0)));
    for frame in 0..=20u32 {
        let mut inputs = Inputs::new()
            .body(first, &low)
            .body(second, &high)
            .drying(0.0);
        if frame == 0 {
            inputs = inputs.emit(
                Emission::new(0, Nozzle::Blob)
                    .at(Vec3::new(0.2, 0.065, 0.0))
                    .volume(4.0e-6),
            );
        }
        fluid
            .advance_to(f64::from(frame) / 60.0, &inputs)
            .expect("a frame");
    }
    let wet_before = wet_amount(&fluid, second);
    assert!(wet_before > 0.0, "the second sheet never got wet");
    fluid.remove_body(first).expect("the first body goes");
    assert!(fluid.wetness(first).is_none());
    assert!(fluid.read_wetness(first).is_err());
    assert_eq!(fluid.read_wetness(second).expect("wetness").len(), 49);
    for frame in 21..=30u32 {
        fluid
            .advance_to(
                f64::from(frame) / 60.0,
                &Inputs::new().body(second, &high).drying(0.0),
            )
            .expect("a frame");
    }
    let wet_after = wet_amount(&fluid, second);
    assert!(
        wet_after >= wet_before * 0.99,
        "wetness fell from {wet_before} to {wet_after}"
    );
    let particles = fluid.read_particles().expect("particles");
    assert!(
        particles
            .iter()
            .filter(|particle| (particle.position.x - 0.2).abs() < 0.035
                && particle.position.z.abs() < 0.035)
            .all(|particle| particle.position.y > 0.05)
    );
}

#[test]
fn spray_bounces_off_body() {
    let mut fluid = crate::support::fluid(crate::support::small());
    fluid
        .set_substances(&[crate::support::water()])
        .expect("water");
    let sheet = crate::support::sheet("sheet", 0.3, 4);
    let body = fluid.add_body(&sheet).expect("a sheet");
    let pose = crate::support::placed(&sheet, Mat4::IDENTITY);
    let mut spray_count = 0usize;
    for frame in 0..=30u32 {
        let mut inputs = Inputs::new().body(body, &pose);
        if frame < 20 {
            inputs = inputs.emit(
                Emission::new(0, Nozzle::Stream { radius: 0.006 })
                    .at(Vec3::new(0.0, 0.1, 0.0))
                    .velocity(Vec3::new(0.0, -3.0, 0.0))
                    .rate(1.0e-4),
            );
        }
        fluid
            .advance_to(f64::from(frame) / 60.0, &inputs)
            .expect("a frame");
        let (_, spray) = crate::support::surface_bytes(&fluid.surface());
        let floats = crate::support::floats(&spray);
        for record in floats.as_chunks::<8>().0 {
            if record[0].abs() < 0.25 && record[2].abs() < 0.25 {
                spray_count += 1;
                assert!(
                    record[1] > -0.002,
                    "spray at {:?} passed through the sheet",
                    &record[..3]
                );
            }
        }
    }
    assert!(spray_count > 0, "the stream threw no spray");
}

fn wet_amount(fluid: &rikka_physics_fluid::Fluid, body: rikka_physics_fluid::BodyId) -> f32 {
    let wetness = fluid.read_wetness(body).expect("wetness");
    let wetting = fluid.wetness(body).expect("shown wetness");
    wetting.ready.wait().expect("wetness is made");
    let shown = crate::support::floats(&crate::support::read(
        &wetting.buffer,
        wetting.buffer.size(),
    ));
    assert_eq!(wetness.as_flattened(), &shown[..4 * wetness.len()]);
    wetness.iter().map(|wet| wet[3]).sum()
}

fn drape(positions: &[Vec3], blend: f32) -> ComputeBuffer {
    let bytes: Vec<u8> = positions
        .iter()
        .flat_map(|at| {
            [
                at.x, at.y, at.z, blend, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0,
            ]
        })
        .flat_map(f32::to_le_bytes)
        .collect();
    assert_eq!(
        bytes.len() as u64,
        positions.len() as u64 * DRAPE_VERTEX_BYTES
    );
    let compute = crate::support::compute();
    let buffer = compute.buffer(bytes.len() as u64).expect("a drape");
    let mut batch = compute.batch();
    batch.write(&buffer, 0, &bytes);
    batch.submit().expect("the drape goes up");
    buffer
}

#[test]
fn drape_moves_fluid_contact_surface() {
    let sheet = crate::support::sheet("sheet", 0.04, 6);
    let raised: Vec<Vec3> = sheet.meshes[0]
        .vertices
        .positions
        .iter()
        .map(|at| *at + Vec3::Y * 0.04)
        .collect();
    let mut fluid = crate::support::fluid(crate::support::small());
    fluid
        .set_substances(&[crate::support::honey()])
        .expect("honey");
    let body = fluid.add_body(&sheet).expect("a sheet");
    let pose = crate::support::placed(&sheet, Mat4::IDENTITY);
    let lifted = drape(&raised, 1.0);
    for frame in 0..=30u32 {
        let mut inputs = Inputs::new()
            .body(body, &pose)
            .drape(body, NodeId(0), &lifted);
        if frame == 0 {
            inputs = inputs.emit(
                Emission::new(0, Nozzle::Blob)
                    .at(Vec3::new(0.0, 0.06, 0.0))
                    .volume(4.0e-6),
            );
        }
        fluid
            .advance_to(f64::from(frame) / 60.0, &inputs)
            .expect("a frame");
    }
    let lowest = crate::support::lowest(&fluid.read_particles().expect("particles"));
    assert!(
        lowest > 0.04,
        "honey fell to {lowest} m, under the draped sheet"
    );
}

#[test]
fn rejects_short_or_unposed_drape() {
    let sheet = crate::support::sheet("sheet", 0.04, 6);
    let positions = &sheet.meshes[0].vertices.positions;
    let mut fluid = crate::support::fluid(crate::support::small());
    let body = fluid.add_body(&sheet).expect("a sheet");
    let pose = crate::support::placed(&sheet, Mat4::IDENTITY);
    let short = drape(&positions[1..], 1.0);
    let refused = fluid.advance_to(
        1.0 / 60.0,
        &Inputs::new()
            .body(body, &pose)
            .drape(body, NodeId(0), &short),
    );
    assert!(
        matches!(refused, Err(Error::DrapeTooSmall { .. })),
        "{refused:?}"
    );
    let whole = drape(positions, 1.0);
    let unposed = fluid.advance_to(1.0 / 60.0, &Inputs::new().drape(body, NodeId(0), &whole));
    assert!(
        matches!(unposed, Err(Error::UnposedDrape(_))),
        "{unposed:?}"
    );
}
