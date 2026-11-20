use std::sync::Arc;

use animate_body::{Body, HandPreset, Side};
use rikka_humanoid::Anatomy;
use rikka_mmd::Puppet;
use rikka_model_core::Instance;

use crate::support::miku;

#[test]
fn pinch_points_mirror_beyond_palms() {
    let model = miku();
    let body =
        Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("the sample is humanoid");
    let rest = body.fk(body.rest_pose());
    let worlds = model.world_transforms();
    let pinches = Side::BOTH.map(|side| {
        let pinch = body.pinch(side).expect("the sample has hands");
        let hand = rest.joint(side.hand());
        let node = body.mapping().node(side.hand()).expect("a mapped hand");
        let placed = worlds[node.index()].w_axis.truncate();
        assert!(
            placed.distance(hand.position) < 1.0e-3,
            "{side:?} hand sits where the model puts it"
        );
        assert!(hand.point(pinch.local).distance(pinch.rest) < 1.0e-4);
        let reach = pinch.rest.distance(hand.position);
        assert!(
            (0.03..0.25).contains(&reach),
            "{side:?} pinch {reach} m from the wrist"
        );
        pinch.rest
    });
    let [left, right] = pinches;
    assert!((left.x + right.x).abs() < 0.01, "{left} mirrors {right}");
    assert!((left.y - right.y).abs() < 0.01 && (left.z - right.z).abs() < 0.01);
}

#[test]
fn write_fingers_affects_one_hand() {
    let model = miku();
    let body =
        Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("the sample is humanoid");
    let mut instance = Instance::new(Arc::new(model));
    if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
        body.release(puppet);
    }
    instance.refresh();
    let place = |instance: &Instance, bone| {
        let node = body.mapping().node(bone).expect("a mapped bone");
        instance.world(node).w_axis.truncate()
    };
    let [left, right] = Side::BOTH.map(|side| side.finger_bones()[5]);
    let before = (place(&instance, left), place(&instance, right));
    let mut pose = *body.rest_pose();
    HandPreset::Fist.apply(Side::Left, &mut pose);
    HandPreset::Fist.apply(Side::Right, &mut pose);
    body.write_fingers(&pose, &[Side::Left], instance.pose_mut());
    instance.refresh();
    assert!(
        place(&instance, left).distance(before.0) > 0.01,
        "the left fingers curl"
    );
    assert!(
        place(&instance, right).distance(before.1) < 1.0e-5,
        "the right hand stays open"
    );
}
