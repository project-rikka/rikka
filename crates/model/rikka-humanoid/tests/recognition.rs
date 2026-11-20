use std::sync::{Arc, OnceLock};

use rikka_humanoid::Anatomy;
use rikka_mmd::{Pmx, PmxImporter};
use rikka_model_core::glam::{Mat4, Vec3};
use rikka_model_core::{HumanBone, Import, Model, Node, NodeId, Registry, Transform};
use rikka_vrm::{Vrm, VrmImporter};

const MIKU: &[u8] = include_bytes!("../../rikka-mmd/assets/mmd/miku.pmx");
const AVATAR: &[u8] = include_bytes!("../../rikka-vrm/assets/avatar.vrm");
const METACARPAL_SHARE: f32 = 0.45;
const UPPER_CHEST_SHARE: f32 = 0.3;

fn read(bytes: &[u8], name: &str) -> Model {
    Import::named_bytes(
        name,
        bytes.to_vec(),
        Arc::new(
            Registry::new()
                .with_importer(PmxImporter)
                .with_importer(VrmImporter),
        ),
    )
    .run()
    .unwrap_or_else(|error| panic!("{name} cannot be read: {error}"))
}

fn miku() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| read(MIKU, "miku.pmx"))
}

fn avatar() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| read(AVATAR, "avatar.vrm"))
}

fn bone(model: &Model, name: &str) -> Option<NodeId> {
    let pmx = model.extensions.get::<Pmx>()?;
    pmx.bones
        .iter()
        .find(|bone| bone.name == name)
        .map(|bone| bone.node)
}

fn described(model: &Model, anatomy: &Anatomy) -> String {
    let name = |node: NodeId| model.node(node).name.clone();
    let mut lines: Vec<String> = anatomy
        .humanoid()
        .bones()
        .map(|(bone, node)| format!("{bone} = {}", name(node)))
        .collect();
    lines.extend(
        anatomy
            .twists()
            .map(|(bone, node)| format!("{bone} twist = {}", name(node))),
    );
    lines.join("\n")
}

const STANDARD: &[(HumanBone, &str)] = &[
    (HumanBone::Spine, "上半身"),
    (HumanBone::Chest, "上半身2"),
    (HumanBone::Neck, "首"),
    (HumanBone::Head, "頭"),
    (HumanBone::LeftEye, "左目"),
    (HumanBone::RightEye, "右目"),
    (HumanBone::LeftShoulder, "左肩"),
    (HumanBone::LeftUpperArm, "左腕"),
    (HumanBone::LeftLowerArm, "左ひじ"),
    (HumanBone::LeftHand, "左手首"),
    (HumanBone::RightShoulder, "右肩"),
    (HumanBone::RightUpperArm, "右腕"),
    (HumanBone::RightLowerArm, "右ひじ"),
    (HumanBone::RightHand, "右手首"),
    (HumanBone::LeftUpperLeg, "左足"),
    (HumanBone::LeftLowerLeg, "左ひざ"),
    (HumanBone::LeftFoot, "左足首"),
    (HumanBone::RightUpperLeg, "右足"),
    (HumanBone::RightLowerLeg, "右ひざ"),
    (HumanBone::RightFoot, "右足首"),
    (HumanBone::LeftToes, "左足先EX"),
    (HumanBone::RightToes, "右足先EX"),
    (HumanBone::LeftThumbMetacarpal, "左親指０"),
    (HumanBone::LeftThumbProximal, "左親指１"),
    (HumanBone::LeftIndexProximal, "左人指１"),
    (HumanBone::LeftIndexDistal, "左人指３"),
    (HumanBone::LeftMiddleProximal, "左中指１"),
    (HumanBone::LeftRingProximal, "左薬指１"),
    (HumanBone::LeftLittleProximal, "左小指１"),
    (HumanBone::RightLittleDistal, "右小指３"),
];

fn assert_standard(model: &Model, anatomy: &Anatomy, context: &str) {
    let report = described(model, anatomy);
    assert_eq!(
        anatomy.humanoid().missing().count(),
        0,
        "{context}\n{report}"
    );
    for (human, name) in STANDARD {
        let expected = bone(model, name);
        assert!(expected.is_some(), "the sample has no {name}");
        assert_eq!(
            anatomy.node(*human),
            expected,
            "{context}: {human} is {name}\n{report}"
        );
    }
}

#[test]
fn pmx_skeleton_recognized() {
    let model = miku();
    let anatomy = Anatomy::recognize(model);
    let report = described(model, &anatomy);
    assert_standard(model, &anatomy, "miku");
    let spine = anatomy.node(HumanBone::Spine).expect("a spine");
    let left_leg = anatomy.node(HumanBone::LeftUpperLeg).expect("a left leg");
    let hips = anatomy.node(HumanBone::Hips).expect("hips");
    let above = |node: NodeId, ancestor: NodeId| {
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            if current == ancestor {
                return true;
            }
            cursor = model.node(current).parent;
        }
        false
    };
    assert!(above(spine, hips) && above(left_leg, hips), "{report}");
    assert_eq!(
        anatomy.twist(HumanBone::LeftUpperArm),
        bone(model, "左腕捩"),
        "{report}"
    );
    assert_eq!(
        anatomy.twist(HumanBone::RightLowerArm),
        bone(model, "右手捩"),
        "{report}"
    );
    assert!(anatomy.forward().z > 0.9, "{:?}", anatomy.forward());
}

#[test]
fn structure_alone_finds_named_humanoid() {
    let model = miku();
    let mut renamed = model.clone();
    for (index, node) in renamed.nodes.iter_mut().enumerate() {
        node.name = format!("node {index}");
    }
    if let Some(pmx) = renamed.extensions.get_mut::<Pmx>() {
        for (index, bone) in pmx.bones.iter_mut().enumerate() {
            bone.name = format!("bone {index}");
            bone.english_name = String::new();
        }
    }
    let original = Anatomy::recognize(model);
    let blind = Anatomy::recognize(&renamed);
    assert_eq!(original.humanoid(), blind.humanoid());
    assert_eq!(
        original.twists().collect::<Vec<_>>(),
        blind.twists().collect::<Vec<_>>()
    );
    assert_eq!(original.swings(), blind.swings());
}

#[derive(Clone, Copy, Debug)]
enum EyeRig {
    HelpersBelow,
    HelpersBelowWithoutGazeControl,
    HeavierPairBeside,
}

fn rigged_eyes(model: &Model, rig: EyeRig) -> Model {
    let mut rigged = model.clone();
    let head = bone(model, "頭").expect("a sample head");
    let worlds = model.world_transforms();
    for (eye, helper) in [("左目", "左目先"), ("右目", "右目先")] {
        let eye = bone(model, eye).expect("a sample eye");
        let helper = bone(model, helper).expect("a sample eye tip");
        if let EyeRig::HeavierPairBeside = rig {
            rigged.nodes[eye.index()]
                .children
                .retain(|child| *child != helper);
            rigged.nodes[head.index()].children.push(helper);
            rigged.nodes[helper.index()].parent = Some(head);
            rigged.nodes[helper.index()].transform =
                Transform::from_matrix(worlds[head.index()].inverse() * worlds[helper.index()]);
        }
        for node in &model.nodes {
            let (Some(mesh), Some(skin)) = (node.mesh, node.skin) else {
                continue;
            };
            let joints = &model.skin(skin).joints;
            let joint = |target: NodeId| {
                joints
                    .iter()
                    .position(|joint| *joint == target)
                    .and_then(|index| u16::try_from(index).ok())
                    .expect("the skin lists every bone")
            };
            let (from, to) = (joint(eye), joint(helper));
            let vertices = &mut rigged.meshes[mesh.index()].vertices;
            for (index, influences) in vertices.joints.iter_mut().enumerate() {
                let moved = match rig {
                    EyeRig::HeavierPairBeside => index % 3 != 0,
                    _ => true,
                };
                for slot in influences.iter_mut().filter(|slot| **slot == from && moved) {
                    *slot = to;
                }
            }
        }
    }
    if let EyeRig::HelpersBelowWithoutGazeControl = rig
        && let Some(pmx) = rigged.extensions.get_mut::<Pmx>()
    {
        for eye in pmx
            .bones
            .iter_mut()
            .filter(|bone| bone.name == "左目" || bone.name == "右目")
        {
            eye.append = None;
        }
    }
    rigged
}

#[test]
fn eyes_are_the_pivots_of_their_flesh() {
    let model = miku();
    for rig in [
        EyeRig::HelpersBelow,
        EyeRig::HelpersBelowWithoutGazeControl,
        EyeRig::HeavierPairBeside,
    ] {
        let rigged = rigged_eyes(model, rig);
        let anatomy = Anatomy::recognize(&rigged);
        let report = described(&rigged, &anatomy);
        for (eye, name) in [(HumanBone::LeftEye, "左目"), (HumanBone::RightEye, "右目")] {
            assert_eq!(
                anatomy.node(eye),
                bone(model, name),
                "{rig:?}: {eye} is {name}\n{report}"
            );
        }
    }
}

fn interposed(model: &Model, children: &[&str], share: f32) -> Model {
    let mut changed = model.clone();
    let worlds = model.world_transforms();
    let children: Vec<NodeId> = children
        .iter()
        .map(|name| bone(model, name).expect("a sample bone"))
        .collect();
    let parent = model.node(children[0]).parent.expect("a parented bone");
    let from = worlds[parent.index()].w_axis.truncate();
    let toward = children
        .iter()
        .map(|child| worlds[child.index()].w_axis.truncate())
        .sum::<Vec3>()
        / children.len() as f32;
    let (_, turn, _) = worlds[parent.index()].to_scale_rotation_translation();
    let world = Mat4::from_rotation_translation(turn, from + (toward - from) * share);
    let inserted = NodeId(changed.nodes.len() as u32);
    let mut node = Node::new(format!("inserted {}", inserted.index()));
    node.parent = Some(parent);
    node.transform = Transform::from_matrix(worlds[parent.index()].inverse() * world);
    node.children.clone_from(&children);
    changed.nodes.push(node);
    let above = &mut changed.nodes[parent.index()].children;
    above.retain(|child| !children.contains(child));
    above.push(inserted);
    for child in &children {
        let moved = &mut changed.nodes[child.index()];
        moved.parent = Some(inserted);
        moved.transform = Transform::from_matrix(world.inverse() * worlds[child.index()]);
    }
    changed
}

#[test]
fn standard_bones_survive_common_rig_variants() {
    let model = miku();
    let upper_chest_below_arms = interposed(model, &["左肩P", "右肩P"], UPPER_CHEST_SHARE);
    let metacarpals = ["人指", "中指", "薬指", "小指"]
        .iter()
        .flat_map(|finger| [format!("左{finger}１"), format!("右{finger}１")])
        .fold(model.clone(), |changed, root| {
            interposed(&changed, &[root.as_str()], METACARPAL_SHARE)
        });
    for (variant, rigged) in [
        ("upper chest below the arms", upper_chest_below_arms),
        ("metacarpal bones", metacarpals),
    ] {
        assert_standard(&rigged, &Anatomy::recognize(&rigged), variant);
    }
}

#[test]
fn nodes_map_to_moving_part() {
    let model = miku();
    let anatomy = Anatomy::recognize(model);
    let part = |name: &str| bone(model, name).and_then(|node| anatomy.part(node));
    assert_eq!(part("下半身"), Some(HumanBone::Hips));
    assert_eq!(part("左腕捩"), Some(HumanBone::LeftUpperArm));
    assert_eq!(part("右足D"), part("右足"));
    assert_eq!(part("右足D"), Some(HumanBone::RightUpperLeg));
    assert_eq!(part("センター"), Some(HumanBone::Hips));
}

#[test]
fn swing_chains_anchor_to_parent_part() {
    let model = miku();
    let anatomy = Anatomy::recognize(model);
    assert!(!anatomy.swings().is_empty());
    let hair = anatomy
        .swings()
        .iter()
        .filter(|swing| swing.bone() == Some(HumanBone::Head))
        .count();
    let hips = anatomy
        .swings()
        .iter()
        .filter(|swing| swing.bone() == Some(HumanBone::Hips))
        .count();
    assert!(hair >= 2, "hair chains hang from the head");
    assert!(hips >= 4, "skirt chains hang from the hips");
    for swing in anatomy.swings() {
        for node in swing.nodes() {
            assert_eq!(anatomy.swing(*node), Some(swing));
            assert_eq!(anatomy.part(*node), None);
        }
    }
}

#[test]
fn breasts_are_flesh_tie_is_not() {
    let model = miku();
    let anatomy = Anatomy::recognize(model);
    let flesh = |name: &str| {
        let node = bone(model, name).expect("a sample bone");
        anatomy.swing(node).map(|swing| swing.is_flesh())
    };
    assert_eq!(flesh("左胸"), Some(true));
    assert_eq!(flesh("右胸"), Some(true));
    assert_eq!(flesh("ネクタイ2"), Some(false));
    assert_eq!(flesh("左髮1"), Some(false));
    assert_eq!(
        anatomy
            .swings()
            .iter()
            .filter(|swing| swing.is_flesh())
            .count(),
        2
    );
}

#[test]
fn avatar_keeps_declared_humanoid() {
    let model = avatar();
    let anatomy = Anatomy::recognize(model);
    let vrm = model.extensions.get::<Vrm>().expect("an avatar");
    assert_eq!(anatomy.humanoid(), &vrm.humanoid);
    assert!(anatomy.forward().z > 0.9, "{:?}", anatomy.forward());
    assert!(
        anatomy
            .swings()
            .iter()
            .any(|swing| swing.bone() == Some(HumanBone::Head))
    );
}

#[test]
fn boneless_model_has_no_humanoid() {
    let anatomy = Anatomy::recognize(&Model::default());
    assert!(anatomy.humanoid().is_empty());
    assert!(anatomy.swings().is_empty());
}
