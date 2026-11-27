use animate_body::Body;
use rikka_humanoid::Anatomy;
use rikka_model_core::HumanBone;

const FINGERS: [HumanBone; 6] = [
    HumanBone::LeftIndexProximal,
    HumanBone::LeftIndexDistal,
    HumanBone::LeftLittleDistal,
    HumanBone::RightThumbDistal,
    HumanBone::RightMiddleIntermediate,
    HumanBone::RightRingProximal,
];

#[test]
fn mannequin_maps_every_required_bone() {
    let model = animate_mannequin::model();
    let anatomy = Anatomy::recognize(&model);
    let humanoid = anatomy.humanoid();
    assert_eq!(humanoid.missing().collect::<Vec<_>>(), Vec::new());
    for bone in [
        HumanBone::Chest,
        HumanBone::UpperChest,
        HumanBone::Neck,
        HumanBone::LeftShoulder,
        HumanBone::RightToes,
    ]
    .into_iter()
    .chain(FINGERS)
    {
        assert!(humanoid.bone(bone).is_some(), "{bone:?} is unmapped");
    }
}

#[test]
fn mannequin_builds_body_standing_on_floor() {
    let model = animate_mannequin::model();
    let anatomy = Anatomy::recognize(&model);
    let body = Body::from_anatomy(&model, &anatomy).expect("a body");
    let height = model.meshes[0].bounds().size().y;
    assert!((1.6..1.85).contains(&height), "height {height}");
    assert!(
        (0.85..1.05).contains(&body.scale()),
        "hips {}",
        body.scale()
    );
    assert!(anatomy.forward().z > 0.9);
}
