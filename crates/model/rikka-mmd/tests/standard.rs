use std::sync::Arc;

use rikka_mmd::{FRAMES_PER_SECOND, Pmx, Puppet, Vmd, VmdImporter, standard_skeleton};
use rikka_model_core::glam::Vec3;
use rikka_model_core::{Import, Instance, Model, Registry};

const MOTION: &[u8] = include_bytes!("../assets/mmd/motion.vmd");
const ARM_REST_DEGREES: f32 = 40.0;
const REACHED: f32 = 0.01;

fn place(model: &Model, instance: &Instance, bone: &str) -> Vec3 {
    let node = model.find_node(bone).expect("a standard bone");
    instance.world(node).w_axis.truncate()
}

#[test]
fn standard_skeleton_stands_in_mmd_rest_pose() {
    let model = standard_skeleton();
    let pmx = model.extensions.get::<Pmx>().expect("a PMX skeleton");
    assert!(pmx.bones.iter().any(|bone| bone.ik.is_some()));
    let instance = Instance::new(Arc::new(model.clone()));
    let arm = place(&model, &instance, "左ひじ") - place(&model, &instance, "左腕");
    let below = (-arm.y).atan2(arm.x).to_degrees();
    assert!((below - ARM_REST_DEGREES).abs() < 0.01, "{below}");
    assert!(
        place(&model, &instance, "左腕").x > 0.0,
        "the left arm is on +X"
    );
    let toe = place(&model, &instance, "左つま先");
    assert!(toe.y.abs() < 1e-5, "{toe}");
    assert!(
        toe.z > place(&model, &instance, "左足首").z,
        "the feet point to +Z"
    );
}

#[test]
fn standard_skeleton_plays_motion_with_reaching_legs() {
    let mut motion = Import::named_bytes(
        "motion.vmd",
        MOTION.to_vec(),
        Arc::new(Registry::new().with_importer(VmdImporter)),
    )
    .run()
    .expect("the sample motion reads");
    let vmd = motion.extensions.remove::<Vmd>().expect("a VMD motion");
    let model = Arc::new(standard_skeleton());
    let animation = vmd.animation(&model);
    let switches = vmd.switches(&model);
    let mut instance = Instance::new(Arc::clone(&model));
    for frame in (0..=vmd.last_frame()).step_by(5) {
        let time = frame as f32 / FRAMES_PER_SECOND;
        instance.pose_mut().reset(&model);
        animation.apply(time, instance.pose_mut());
        if let Some(puppet) = instance.rig_as_mut::<Puppet>() {
            switches.apply(time, puppet);
        }
        instance.refresh();
        assert!(
            instance.worlds().iter().all(|world| world.is_finite()),
            "frame {frame}"
        );
        for side in ["左", "右"] {
            let ankle = place(&model, &instance, &format!("{side}足首"));
            let goal = place(&model, &instance, &format!("{side}足ＩＫ"));
            let hip = place(&model, &instance, &format!("{side}足"));
            let reach = place(&model, &instance, &format!("{side}ひざ")).distance(hip)
                + ankle.distance(place(&model, &instance, &format!("{side}ひざ")));
            if goal.distance(hip) < reach * 0.98 {
                assert!(
                    ankle.distance(goal) < REACHED,
                    "{side} leg misses its goal by {} at frame {frame}",
                    ankle.distance(goal)
                );
            }
        }
    }
}
