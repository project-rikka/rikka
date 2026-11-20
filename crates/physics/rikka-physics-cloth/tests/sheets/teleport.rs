use crate::support::Sheet;
use glam::{Mat4, Vec3};
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::PieceKind;

#[test]
fn teleport_keeps_cloth_intact() {
    let sheet = Sheet::new(8, 10, 0.02, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let mut instance = sheet.instance();
    let rest = sheet.rest();
    for frame in 0..90 {
        let placement = if frame < 30 {
            Mat4::IDENTITY
        } else {
            Mat4::from_translation(Vec3::new(40.0, -3.0, 25.0))
        };
        instance.set_transform(placement);
        crate::support::step(&mut cloth, figure, &instance, frame);
        if frame >= 30 {
            let positions = crate::support::positions(&mut cloth, figure);
            assert!(positions.iter().all(|p| p.is_finite()));
            let farthest = positions
                .iter()
                .zip(&rest)
                .map(|(position, rest)| position.distance(placement.transform_point3(*rest)))
                .fold(0.0, f32::max);
            assert!(
                farthest < 0.15,
                "frame {frame}: a particle strayed {farthest} m"
            );
            let stretch = crate::support::longest_stretch(&sheet, &positions);
            assert!(stretch < 1.12, "frame {frame}: stretched {stretch}");
        }
    }
}
