use std::sync::Arc;

use glam::Vec3;
use rikka_model_core::Instance;
use rikka_physics_cloth::{Cloth, FigureId, Inputs};
use rikka_physics_tailor::{Grab, PieceKind};

use crate::support::Sheet;

const HELD: u64 = 7;

fn held_step(
    cloth: &mut Cloth,
    figure: FigureId,
    instance: &Instance,
    frame: u32,
    grab: Option<&Grab>,
) {
    let time = f64::from(frame) / f64::from(cloth.rate());
    let inputs = Inputs::new().figure(figure, instance);
    let inputs = match grab {
        Some(grab) => inputs.grab(figure, grab.clone()),
        None => inputs,
    };
    cloth.advance(time, &inputs).expect("a step");
}

#[test]
fn grabbed_sheet_follows_hand_until_release() {
    let sheet = Sheet::new(10, 12, 0.02, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let instance = sheet.instance();
    let corner = (sheet.rows * sheet.columns - sheet.columns / 2) as usize;
    let rest = sheet.rest()[corner];
    let pick = sheet
        .asset
        .pick(rest, 0.03)
        .expect("the sheet lies under the hand");
    assert_eq!(pick.centre, corner);
    let point = rest + Vec3::new(0.0, 0.1, 0.08);
    let grab = Grab {
        id: HELD,
        pick: Arc::new(pick),
        point,
        strength: 1.0,
    };
    for frame in 0..120 {
        held_step(&mut cloth, figure, &instance, frame, Some(&grab));
    }
    let positions = crate::support::positions(&mut cloth, figure);
    assert!(positions.iter().all(|at| at.is_finite()));
    assert!(
        positions[corner].distance(point) < 0.01,
        "{:?} held at {point:?}",
        positions[corner]
    );

    for frame in 120..480 {
        held_step(&mut cloth, figure, &instance, frame, None);
    }
    let positions = crate::support::positions(&mut cloth, figure);
    assert!(
        positions[corner].distance(rest) < 0.03,
        "{:?} hangs back at {rest:?}",
        positions[corner]
    );
}

#[test]
fn grab_holds_cloth_beyond_reach() {
    let sheet = Sheet::new(10, 12, 0.02, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let instance = sheet.instance();
    let corner = (sheet.rows * sheet.columns - sheet.columns / 2) as usize;
    let rest = sheet.rest()[corner];
    let pick = sheet.asset.pick(rest, 0.03).expect("cloth under the hand");
    let mut grab = Grab {
        id: HELD,
        pick: Arc::new(pick),
        point: rest,
        strength: 1.0,
    };
    for frame in 0..240 {
        grab.point = rest + Vec3::new(0.0, 0.0, 0.6 * (frame as f32 / 120.0).min(1.0));
        held_step(&mut cloth, figure, &instance, frame, Some(&grab));
    }
    let positions = crate::support::positions(&mut cloth, figure);
    assert!(positions.iter().all(|at| at.is_finite()));
    assert!(
        positions[corner].distance(grab.point) < 0.01,
        "{:?} held at {:?}",
        positions[corner],
        grab.point
    );
}
