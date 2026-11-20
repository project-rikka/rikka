use std::f32::consts::{FRAC_PI_2, PI};

use crate::support::Sheet;
use glam::Mat4;
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::{PieceKind, Style};

const TURNED_OVER: f32 = 10.0;

fn lowest_after(change: Option<Style>) -> f32 {
    let sheet = Sheet::new(8, 10, 0.02, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let mut instance = sheet.instance();
    for frame in 0..16 {
        if frame == 2
            && let Some(style) = &change
        {
            cloth.set_style(figure, 0, style.clone()).expect("a style");
        }
        let turned = ((frame as f32 - 1.0) / TURNED_OVER).clamp(0.0, 1.0);
        instance.set_transform(Mat4::from_rotation_x(-FRAC_PI_2 + PI * turned));
        crate::support::step(&mut cloth, figure, &instance, frame);
    }
    crate::support::positions(&mut cloth, figure)
        .iter()
        .map(|p| p.y)
        .fold(0.0, f32::min)
}

#[test]
fn style_change_alters_motion() {
    let unchanged = lowest_after(None);
    let lighter = lowest_after(Some(Style {
        gravity: 0.2,
        ..crate::support::style()
    }));
    let springier = lowest_after(Some(Style {
        restore: rikka_physics_tailor::Curve::flat(0.3),
        ..crate::support::style()
    }));
    assert!(
        (lighter - unchanged).abs() > 0.02,
        "{lighter} against {unchanged}"
    );
    assert!(
        (springier - unchanged).abs() > 0.02,
        "{springier} against {unchanged}"
    );
}

#[test]
fn rejects_style_for_missing_piece() {
    let sheet = Sheet::new(3, 3, 0.02, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    assert!(cloth.set_style(figure, 1, crate::support::style()).is_err());
    assert!(cloth.add_styled_figure(sheet.asset.clone(), []).is_err());
}
