use std::sync::{Arc, LazyLock};

use animate_body::{Body, HumanPose, Side};
use rikka_humanoid::Anatomy;
use rikka_mmd::PmxImporter;
use rikka_model_core::{Import, Registry};

const MIKU: &[u8] =
    include_bytes!("../../../../../../../crates/model/rikka-mmd/assets/mmd/miku.pmx");

pub(crate) static BODY: LazyLock<Arc<Body>> = LazyLock::new(|| {
    let model = Import::named_bytes(
        "miku.pmx",
        MIKU.to_vec(),
        Arc::new(Registry::new().with_importer(PmxImporter)),
    )
    .run()
    .expect("the sample model reads");
    Arc::new(Body::from_anatomy(&model, &Anatomy::recognize(&model)).expect("miku is human"))
});

pub(crate) fn rest() -> HumanPose {
    *BODY.rest_pose()
}

pub(crate) fn changed(pose: &HumanPose, after: &HumanPose) -> Vec<usize> {
    (0..pose.muscles.len())
        .filter(|index| (after.muscles[*index] - pose.muscles[*index]).abs() > 1e-5)
        .collect()
}

pub(crate) fn feet_stay(body: &Body, pose: &HumanPose, result: &HumanPose, name: &str) {
    let (before, after) = (body.fk(pose), body.fk(result));
    for side in Side::BOTH {
        let foot = side.foot();
        let slid = after.position(foot).distance(before.position(foot));
        assert!(slid < 0.001, "{name}: the {side:?} foot slid {slid} m");
        let angle = after.rotation(foot).angle_between(before.rotation(foot));
        assert!(angle < 0.01, "{name}: the {side:?} foot turned {angle} rad");
    }
}
