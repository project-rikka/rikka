use std::sync::Arc;

use rikka_model::glam::{Quat, Vec3};
use rikka_model::humanoid::Anatomy;
use rikka_model::{HumanBone, Import, Instance, Model, registry};
use rikka_model_gpu::HeadRest;

const MIKU: &[u8] = include_bytes!("../../rikka-mmd/assets/mmd/miku.pmx");
const AVATAR: &[u8] = include_bytes!("../../rikka-vrm/assets/avatar.vrm");
const FACING: f32 = 0.95;
const TURNED: f32 = 0.999;

fn read(bytes: &[u8], name: &str) -> Arc<Model> {
    let model = Import::named_bytes(name, bytes.to_vec(), registry())
        .run()
        .unwrap_or_else(|error| panic!("{name} cannot be read: {error}"));
    Arc::new(model)
}

fn eyes(model: &Model, instance: &Instance) -> Vec3 {
    let anatomy = Anatomy::recognize(model);
    let eye = |bone| {
        let node = anatomy.node(bone).expect("both eyes are recognized");
        instance.world(node).w_axis.truncate()
    };
    (eye(HumanBone::LeftEye) + eye(HumanBone::RightEye)) * 0.5
}

fn turn_head(instance: &mut Instance, model: &Model, turn: Quat) {
    let anatomy = Anatomy::recognize(model);
    let head = anatomy.node(HumanBone::Head).expect("a head");
    let parent = model.node(head).parent.expect("a head has a parent");
    let above = instance.world(parent).to_scale_rotation_translation().1;
    let mut local = instance.pose().local(head);
    local.rotation = above.inverse() * turn * above * local.rotation;
    instance.pose_mut().set_local(head, local);
    instance.refresh();
}

#[test]
fn head_frame_faces_out_of_the_face() {
    for (bytes, name) in [(MIKU, "miku.pmx"), (AVATAR, "avatar.vrm")] {
        let model = read(bytes, name);
        let head = HeadRest::new(&model, &Anatomy::recognize(&model)).expect("a head");
        let mut instance = Instance::new(Arc::clone(&model));
        instance.refresh();
        let rest = head.place(&instance);
        let gaze = (eyes(&model, &instance) - rest.center)
            .with_y(0.0)
            .normalize();
        assert!(rest.up.dot(Vec3::Y) > FACING, "{name}: up {:?}", rest.up);
        assert!(
            rest.forward.dot(gaze) > FACING,
            "{name}: forward {:?} against eyes {gaze:?}",
            rest.forward
        );

        let turn = Quat::from_rotation_y(0.8) * Quat::from_rotation_x(-0.4);
        turn_head(&mut instance, &model, turn);
        let turned = head.place(&instance);
        assert!(
            turned.forward.dot(turn * rest.forward) > TURNED,
            "{name}: turned forward {:?}",
            turned.forward
        );
        assert!(
            turned.up.dot(turn * rest.up) > TURNED,
            "{name}: turned up {:?}",
            turned.up
        );
    }
}
