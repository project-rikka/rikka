use std::f32::consts::FRAC_PI_2;

use crate::support::Sheet;
use glam::Mat4;
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::PieceKind;

#[test]
fn pinned_sheet_hangs_with_bounded_stretch() {
    let sheet = Sheet::new(10, 12, 0.02, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let mut instance = sheet.instance();
    instance.set_transform(Mat4::from_rotation_x(-FRAC_PI_2));
    let length = (sheet.rows - 1) as f32 * sheet.spacing;
    let mut lowest_while_falling = 0.0f32;
    for frame in 0..360 {
        crate::support::step(&mut cloth, figure, &instance, frame);
        if frame == 12 {
            let positions = crate::support::positions(&mut cloth, figure);
            lowest_while_falling = positions.iter().map(|p| p.y).fold(0.0, f32::min);
        }
    }
    let positions = crate::support::positions(&mut cloth, figure);
    assert!(positions.iter().all(|p| p.is_finite()));
    assert!(lowest_while_falling < -0.02, "{lowest_while_falling}");
    let lowest = positions.iter().map(|p| p.y).fold(0.0, f32::min);
    assert!(
        lowest < -0.85 * length && lowest > -1.12 * length,
        "{lowest} of {length}"
    );
    let stretch = crate::support::longest_stretch(&sheet, &positions);
    assert!(stretch < 1.12, "{stretch}");
    let rest = sheet.rest();
    for column in 0..sheet.columns as usize {
        let pinned = Mat4::from_rotation_x(-FRAC_PI_2).transform_point3(rest[column]);
        assert!(positions[column].distance(pinned) < 1e-5);
    }
    assert!(cloth.motion() < 0.08, "{}", cloth.motion());
}
