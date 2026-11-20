use std::f32::consts::FRAC_PI_2;

use crate::support::Sheet;
use glam::{Mat4, Vec3};
use rikka_physics_cloth::{Cloth, Inputs, REACTION_BYTES, REACTION_UNIT};
use rikka_physics_tailor::PieceKind;

fn drape_bytes(cloth: &Cloth, figure: rikka_physics_cloth::FigureId) -> Vec<u8> {
    let drape = cloth.drapes(figure).expect("drapes").remove(0);
    drape.ready.wait().expect("a drape");
    crate::support::read(&drape.buffer, drape.buffer.size())
}

#[test]
fn shown_capture_matches_captured_step() {
    let sheet = Sheet::new(8, 9, 0.025, PieceKind::Sheet);
    let mut simulating = crate::support::cloth(Cloth::builder());
    let mut showing = crate::support::cloth(Cloth::builder());
    let figure = simulating
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let shown = showing
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    assert_eq!(figure, shown);
    let mut instance = sheet.instance();
    let mut captures = Vec::new();
    for frame in 0..30 {
        instance.set_transform(Mat4::from_rotation_x(-FRAC_PI_2 * (frame as f32 / 30.0)));
        crate::support::step(&mut simulating, figure, &instance, frame);
        if frame % 10 == 9 {
            let capture = simulating
                .capture()
                .expect("a capture")
                .finish()
                .expect("a finished capture");
            assert!(capture.bytes() > 0);
            captures.push((capture, drape_bytes(&simulating, figure)));
        }
    }
    for (capture, expected) in &captures {
        showing.show(capture).expect("a shown capture");
        assert_eq!(&drape_bytes(&showing, shown), expected);
    }
}

#[test]
fn liquid_momentum_pushes_cloth() {
    let sheet = Sheet::new(6, 6, 0.03, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder().gravity(Vec3::ZERO));
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let instance = sheet.instance();
    for frame in 0..10 {
        crate::support::step(&mut cloth, figure, &instance, frame);
    }
    let before = crate::support::positions(&mut cloth, figure);
    let vertices = before.len() as u64;
    let push = (2.0e-4 / REACTION_UNIT) as i32;
    let words: Vec<i32> = (0..vertices).flat_map(|_| [0, 0, push, 0]).collect();
    let compute = crate::support::compute();
    let reaction = compute
        .buffer(vertices * REACTION_BYTES)
        .expect("a reaction buffer");
    let mut batch = compute.batch();
    batch.write(&reaction, 0, bytemuck::cast_slice(&words));
    batch.submit().expect("an upload");
    let node = sheet.asset.pieces[0].bindings[0].node;
    cloth
        .advance(
            10.0 / 60.0,
            &Inputs::new()
                .figure(figure, &instance)
                .reaction(figure, node, &reaction),
        )
        .expect("a pushed step");
    let after = crate::support::positions(&mut cloth, figure);
    let moved = before
        .iter()
        .zip(&after)
        .skip(sheet.columns as usize)
        .map(|(start, end)| end.z - start.z)
        .fold(f32::MAX, f32::min);
    assert!(
        moved > 1.0e-4,
        "the cloth moved only {moved} m with the liquid"
    );
}
