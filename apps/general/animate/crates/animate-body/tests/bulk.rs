use animate_body::{Body, HumanBone};
use rikka_humanoid::Anatomy;
use rikka_model_core::glam::Vec3;

use crate::support::miku;

#[test]
fn inner_point_exits_toward_viewer() {
    let model = miku();
    let body = Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("miku is human");
    let worlds = body.fk(body.rest_pose());
    let bulk = body.bulk();
    let scale = body.scale();
    let gap = 0.02 * scale;
    for (bone, toward) in [
        (HumanBone::Hips, Vec3::Z),
        (HumanBone::Hips, Vec3::NEG_Z),
        (HumanBone::Chest, Vec3::Z),
        (HumanBone::Head, Vec3::X),
        (HumanBone::LeftUpperLeg, Vec3::Z),
    ] {
        let inside = worlds.position(bone);
        assert!(bulk.holds(&worlds, inside, 0.0), "{bone:?} holds its joint");
        let out = bulk.surface(&worlds, inside, toward, gap);
        let walked = (out - inside).dot(toward);
        assert!(!bulk.holds(&worlds, out, gap), "{bone:?} walks out");
        assert!(
            (0.03 * scale..0.3 * scale).contains(&walked),
            "{bone:?} walks {walked} toward {toward}"
        );
        assert!((out - inside).cross(toward).length() < 1e-5);
    }
    let ahead = worlds.position(HumanBone::Hips) + Vec3::Z * scale;
    assert!(!bulk.holds(&worlds, ahead, gap));
    assert_eq!(bulk.surface(&worlds, ahead, Vec3::NEG_Z, gap), ahead);
}
