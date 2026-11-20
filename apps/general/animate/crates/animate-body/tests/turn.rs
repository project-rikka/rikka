use animate_body::{arc, canonical, rotation_vector, twist_about};
use rikka_model_core::glam::{Quat, Vec3};

#[test]
fn quaternion_sign_takes_short_arc() {
    let turn = Quat::from_rotation_y(2.5);
    assert_eq!(canonical(-turn), canonical(turn));
    assert!((twist_about(-turn, Vec3::Y) - 2.5).abs() < 1e-5);
    assert!(rotation_vector(-turn).abs_diff_eq(Vec3::Y * 2.5, 1e-5));
    assert!(arc(Vec3::ZERO, Vec3::X).abs_diff_eq(Quat::IDENTITY, 0.0));
}
