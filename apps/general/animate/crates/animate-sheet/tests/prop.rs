use animate_body::Faces;
use animate_motion::Take;
use animate_prop::{PropKey, PropMotion, PropSetting};
use animate_sheet::{BoneDisplay, GroupKind, Lane, Sheet};
use rikka_mmd::{VmdBoneFrame, VmdCurve, VmdInterpolation};
use rikka_model_core::Model;
use rikka_model_core::glam::{Quat, Vec3};

fn lanes(sheet: &Sheet) -> Vec<(GroupKind, Vec<Lane>)> {
    sheet
        .groups()
        .iter()
        .map(|group| (group.kind.clone(), group.lanes().cloned().collect()))
        .collect()
}

#[test]
fn only_accessories_list_an_attachment_row() {
    let mut motion = PropMotion::default();
    motion.tracks_mut().insert(PropKey {
        setting: PropSetting::Yaw,
        frame: 12,
        value: 90.0,
        curve: VmdCurve::LINEAR,
    });
    let placement: Vec<Lane> = PropSetting::ALL.iter().copied().map(Lane::Prop).collect();
    let stage = Sheet::prop(&motion, false);
    assert_eq!(lanes(&stage), [(GroupKind::Prop, placement.clone())]);
    assert_eq!(stage.keys(), [12]);
    let accessory = Sheet::prop(&motion, true);
    let attached: Vec<Lane> = placement.into_iter().chain([Lane::Anchor]).collect();
    assert_eq!(lanes(&accessory), [(GroupKind::Prop, attached)]);
}

#[test]
fn a_prop_take_lists_only_its_keyed_bones() {
    let take = Take {
        bones: [VmdBoneFrame {
            bone: "Door".into(),
            frame: 30,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            interpolation: VmdInterpolation::default(),
            physics_off: false,
        }]
        .into_iter()
        .collect(),
        ..Take::default()
    };
    let faces = Faces::from_model(&Model::default());
    let sheet = Sheet::prop_with_take(
        &PropMotion::default(),
        false,
        &BoneDisplay::default(),
        &faces,
        &take,
    );
    let groups = lanes(&sheet);
    assert_eq!(groups[0].0, GroupKind::Prop);
    assert_eq!(
        groups[1..]
            .iter()
            .flat_map(|(_, lanes)| lanes.clone())
            .collect::<Vec<Lane>>(),
        [Lane::Bone("Door".into())]
    );
    assert_eq!(sheet.keys(), [30]);
}
