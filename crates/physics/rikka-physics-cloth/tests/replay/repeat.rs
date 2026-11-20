use std::f32::consts::FRAC_PI_2;

use crate::support::Sheet;
use glam::{Mat4, Quat, Vec3};
use rikka_physics_cloth::{Cloth, FigureId, Inputs};
use rikka_physics_tailor::{Curve, Flutter, PieceKind, Style, Wind};

fn windy() -> Style {
    Style {
        drag: 1.0,
        lift: 0.4,
        restore: Curve::linear(0.2, 0.02),
        limit: Curve::linear(0.3, 1.2),
        limit_stiffness: 0.5,
        pull: Curve::flat(0.01),
        flutter: Flutter {
            amplitude: 0.005,
            frequency: 2.0,
            wavelength: 0.3,
        },
        ..crate::support::style()
    }
}

fn placement(frame: u32) -> Mat4 {
    let t = frame as f32 / 60.0;
    Mat4::from_rotation_translation(
        Quat::from_rotation_y(0.8 * (2.0 * t).sin()) * Quat::from_rotation_x(-FRAC_PI_2 * 0.5),
        Vec3::new(0.3 * (3.0 * t).sin(), 0.0, 0.1 * t),
    )
}

fn step(cloth: &mut Cloth, frame: u32, figure: FigureId, sheet: &Sheet) {
    let mut instance = sheet.instance();
    instance.set_transform(placement(frame));
    let inputs = Inputs::new().figure(figure, &instance).wind(Wind {
        direction: Vec3::new(1.0, 0.0, 0.3),
        speed: 3.0,
        turbulence: 0.4,
        gust: 0.5,
    });
    cloth
        .advance(f64::from(frame) / 60.0, &inputs)
        .expect("a step");
}

fn drape_bytes(cloth: &Cloth, figure: FigureId) -> Vec<u8> {
    let drape = cloth.drapes(figure).expect("drapes").remove(0);
    drape.ready.wait().expect("a drape");
    crate::support::read(&drape.buffer, drape.buffer.size())
}

#[test]
fn runs_are_bit_exact() {
    let sheet = Sheet::new(9, 9, 0.025, PieceKind::Sheet);
    let run = || {
        let mut cloth = crate::support::cloth(Cloth::builder());
        let figure = cloth
            .add_styled_figure(sheet.asset.clone(), [windy()])
            .expect("a figure");
        for frame in 0..50 {
            step(&mut cloth, frame, figure, &sheet);
        }
        (
            crate::support::bits(&mut cloth, figure),
            drape_bytes(&cloth, figure),
        )
    };
    let (first, first_drape) = run();
    let (second, second_drape) = run();
    assert!(first.iter().all(|bits| f32::from_bits(*bits).is_finite()));
    assert_eq!(first, second);
    assert_eq!(first_drape, second_drape);
}

#[test]
fn restored_checkpoint_continues_bit_exact() {
    let sheet = Sheet::new(9, 9, 0.025, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder());
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [windy()])
        .expect("a figure");
    for frame in 0..20 {
        step(&mut cloth, frame, figure, &sheet);
    }
    let checkpoint = cloth.checkpoint().expect("a checkpoint");
    for frame in 20..45 {
        step(&mut cloth, frame, figure, &sheet);
    }
    let straight = crate::support::bits(&mut cloth, figure);
    let straight_drape = drape_bytes(&cloth, figure);
    cloth.restore(&checkpoint).expect("a restore");
    assert_eq!(cloth.steps(), checkpoint.steps());
    for frame in 20..45 {
        step(&mut cloth, frame, figure, &sheet);
    }
    assert_eq!(crate::support::bits(&mut cloth, figure), straight);
    assert_eq!(drape_bytes(&cloth, figure), straight_drape);
}
