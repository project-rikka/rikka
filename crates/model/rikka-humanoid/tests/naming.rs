use std::sync::Arc;

use rikka_fbx::FbxImporter;
use rikka_humanoid::Anatomy;
use rikka_model_core::glam::Vec3;
use rikka_model_core::{HumanBone, Import, Model, Node, NodeId, Registry, Transform};

const KNIGHT: &[u8] = include_bytes!("../../rikka-fbx/assets/knight.fbx");

use HumanBone as B;

struct Joint {
    role: &'static str,
    parent: Option<&'static str>,
    at: [f32; 3],
    bone: Option<HumanBone>,
}

const fn joint(
    role: &'static str,
    parent: Option<&'static str>,
    at: [f32; 3],
    bone: Option<HumanBone>,
) -> Joint {
    Joint {
        role,
        parent,
        at,
        bone,
    }
}

const LAYOUT: &[Joint] = &[
    joint("root", None, [0.0, 0.0, 0.0], None),
    joint("hips", Some("root"), [0.0, 1.0, 0.0], Some(B::Hips)),
    joint("pelvis", Some("hips"), [0.0, 0.98, 0.0], None),
    joint("spine", Some("hips"), [0.0, 1.1, 0.0], Some(B::Spine)),
    joint("chest", Some("spine"), [0.0, 1.25, 0.0], Some(B::Chest)),
    joint(
        "upper chest",
        Some("chest"),
        [0.0, 1.4, 0.0],
        Some(B::UpperChest),
    ),
    joint("neck", Some("upper chest"), [0.0, 1.5, 0.0], Some(B::Neck)),
    joint("head", Some("neck"), [0.0, 1.6, 0.0], Some(B::Head)),
    joint("head top", Some("head"), [0.0, 1.8, 0.0], None),
    joint(
        "left eye",
        Some("head"),
        [0.03, 1.68, 0.08],
        Some(B::LeftEye),
    ),
    joint(
        "right eye",
        Some("head"),
        [-0.03, 1.68, 0.08],
        Some(B::RightEye),
    ),
    joint(
        "left shoulder",
        Some("upper chest"),
        [0.05, 1.45, 0.0],
        Some(B::LeftShoulder),
    ),
    joint(
        "left upper arm",
        Some("left shoulder"),
        [0.15, 1.45, 0.0],
        Some(B::LeftUpperArm),
    ),
    joint(
        "left arm twist",
        Some("left upper arm"),
        [0.28, 1.45, 0.0],
        None,
    ),
    joint(
        "left lower arm",
        Some("left arm twist"),
        [0.42, 1.45, 0.0],
        Some(B::LeftLowerArm),
    ),
    joint(
        "left hand",
        Some("left lower arm"),
        [0.68, 1.45, 0.0],
        Some(B::LeftHand),
    ),
    joint(
        "left thumb 1",
        Some("left hand"),
        [0.70, 1.44, 0.03],
        Some(B::LeftThumbMetacarpal),
    ),
    joint(
        "left thumb 2",
        Some("left thumb 1"),
        [0.72, 1.44, 0.05],
        Some(B::LeftThumbProximal),
    ),
    joint(
        "left thumb 3",
        Some("left thumb 2"),
        [0.74, 1.44, 0.06],
        Some(B::LeftThumbDistal),
    ),
    joint(
        "left thumb 4",
        Some("left thumb 3"),
        [0.76, 1.44, 0.07],
        None,
    ),
    joint(
        "left index 1",
        Some("left hand"),
        [0.76, 1.45, 0.02],
        Some(B::LeftIndexProximal),
    ),
    joint(
        "left index 2",
        Some("left index 1"),
        [0.80, 1.45, 0.02],
        Some(B::LeftIndexIntermediate),
    ),
    joint(
        "left index 3",
        Some("left index 2"),
        [0.83, 1.45, 0.02],
        Some(B::LeftIndexDistal),
    ),
    joint(
        "left index 4",
        Some("left index 3"),
        [0.85, 1.45, 0.02],
        None,
    ),
    joint(
        "left little 1",
        Some("left hand"),
        [0.75, 1.45, -0.03],
        Some(B::LeftLittleProximal),
    ),
    joint(
        "left little 2",
        Some("left little 1"),
        [0.78, 1.45, -0.03],
        Some(B::LeftLittleIntermediate),
    ),
    joint(
        "left little 3",
        Some("left little 2"),
        [0.80, 1.45, -0.03],
        Some(B::LeftLittleDistal),
    ),
    joint(
        "right shoulder",
        Some("upper chest"),
        [-0.05, 1.45, 0.0],
        Some(B::RightShoulder),
    ),
    joint(
        "right upper arm",
        Some("right shoulder"),
        [-0.15, 1.45, 0.0],
        Some(B::RightUpperArm),
    ),
    joint(
        "right arm twist",
        Some("right upper arm"),
        [-0.28, 1.45, 0.0],
        None,
    ),
    joint(
        "right lower arm",
        Some("right arm twist"),
        [-0.42, 1.45, 0.0],
        Some(B::RightLowerArm),
    ),
    joint(
        "right hand",
        Some("right lower arm"),
        [-0.68, 1.45, 0.0],
        Some(B::RightHand),
    ),
    joint(
        "right index 1",
        Some("right hand"),
        [-0.76, 1.45, 0.02],
        Some(B::RightIndexProximal),
    ),
    joint(
        "right index 2",
        Some("right index 1"),
        [-0.80, 1.45, 0.02],
        Some(B::RightIndexIntermediate),
    ),
    joint(
        "right index 3",
        Some("right index 2"),
        [-0.83, 1.45, 0.02],
        Some(B::RightIndexDistal),
    ),
    joint(
        "left upper leg",
        Some("pelvis"),
        [0.1, 0.95, 0.0],
        Some(B::LeftUpperLeg),
    ),
    joint(
        "left lower leg",
        Some("left upper leg"),
        [0.1, 0.5, 0.0],
        Some(B::LeftLowerLeg),
    ),
    joint(
        "left foot",
        Some("left lower leg"),
        [0.1, 0.08, 0.0],
        Some(B::LeftFoot),
    ),
    joint(
        "left toes",
        Some("left foot"),
        [0.1, 0.0, 0.12],
        Some(B::LeftToes),
    ),
    joint("left toe end", Some("left toes"), [0.1, 0.0, 0.2], None),
    joint(
        "right upper leg",
        Some("pelvis"),
        [-0.1, 0.95, 0.0],
        Some(B::RightUpperLeg),
    ),
    joint(
        "right lower leg",
        Some("right upper leg"),
        [-0.1, 0.5, 0.0],
        Some(B::RightLowerLeg),
    ),
    joint(
        "right foot",
        Some("right lower leg"),
        [-0.1, 0.08, 0.0],
        Some(B::RightFoot),
    ),
    joint(
        "right toes",
        Some("right foot"),
        [-0.1, 0.0, 0.12],
        Some(B::RightToes),
    ),
];

type Naming = &'static [(&'static str, &'static str)];

const MIXAMO: Naming = &[
    ("root", "Armature"),
    ("hips", "mixamorig:Hips"),
    ("spine", "mixamorig:Spine"),
    ("chest", "mixamorig:Spine1"),
    ("upper chest", "mixamorig:Spine2"),
    ("neck", "mixamorig:Neck"),
    ("head", "mixamorig:Head"),
    ("head top", "mixamorig:HeadTop_End"),
    ("left eye", "mixamorig:LeftEye"),
    ("right eye", "mixamorig:RightEye"),
    ("left shoulder", "mixamorig:LeftShoulder"),
    ("left upper arm", "mixamorig:LeftArm"),
    ("left lower arm", "mixamorig:LeftForeArm"),
    ("left hand", "mixamorig:LeftHand"),
    ("left thumb 1", "mixamorig:LeftHandThumb1"),
    ("left thumb 2", "mixamorig:LeftHandThumb2"),
    ("left thumb 3", "mixamorig:LeftHandThumb3"),
    ("left thumb 4", "mixamorig:LeftHandThumb4"),
    ("left index 1", "mixamorig:LeftHandIndex1"),
    ("left index 2", "mixamorig:LeftHandIndex2"),
    ("left index 3", "mixamorig:LeftHandIndex3"),
    ("left index 4", "mixamorig:LeftHandIndex4"),
    ("left little 1", "mixamorig:LeftHandPinky1"),
    ("left little 2", "mixamorig:LeftHandPinky2"),
    ("left little 3", "mixamorig:LeftHandPinky3"),
    ("right shoulder", "mixamorig:RightShoulder"),
    ("right upper arm", "mixamorig:RightArm"),
    ("right lower arm", "mixamorig:RightForeArm"),
    ("right hand", "mixamorig:RightHand"),
    ("right index 1", "mixamorig:RightHandIndex1"),
    ("right index 2", "mixamorig:RightHandIndex2"),
    ("right index 3", "mixamorig:RightHandIndex3"),
    ("left upper leg", "mixamorig:LeftUpLeg"),
    ("left lower leg", "mixamorig:LeftLeg"),
    ("left foot", "mixamorig:LeftFoot"),
    ("left toes", "mixamorig:LeftToeBase"),
    ("left toe end", "mixamorig:LeftToe_End"),
    ("right upper leg", "mixamorig:RightUpLeg"),
    ("right lower leg", "mixamorig:RightLeg"),
    ("right foot", "mixamorig:RightFoot"),
    ("right toes", "mixamorig:RightToeBase"),
];

const UNREAL: Naming = &[
    ("root", "root"),
    ("hips", "pelvis"),
    ("spine", "spine_01"),
    ("chest", "spine_02"),
    ("upper chest", "spine_03"),
    ("neck", "neck_01"),
    ("head", "head"),
    ("left shoulder", "clavicle_l"),
    ("left upper arm", "upperarm_l"),
    ("left arm twist", "upperarm_twist_01_l"),
    ("left lower arm", "lowerarm_l"),
    ("left hand", "hand_l"),
    ("left thumb 1", "thumb_01_l"),
    ("left thumb 2", "thumb_02_l"),
    ("left thumb 3", "thumb_03_l"),
    ("left index 1", "index_01_l"),
    ("left index 2", "index_02_l"),
    ("left index 3", "index_03_l"),
    ("left little 1", "pinky_01_l"),
    ("left little 2", "pinky_02_l"),
    ("left little 3", "pinky_03_l"),
    ("right shoulder", "clavicle_r"),
    ("right upper arm", "upperarm_r"),
    ("right arm twist", "upperarm_twist_01_r"),
    ("right lower arm", "lowerarm_r"),
    ("right hand", "hand_r"),
    ("right index 1", "index_01_r"),
    ("right index 2", "index_02_r"),
    ("right index 3", "index_03_r"),
    ("left upper leg", "thigh_l"),
    ("left lower leg", "calf_l"),
    ("left foot", "foot_l"),
    ("left toes", "ball_l"),
    ("right upper leg", "thigh_r"),
    ("right lower leg", "calf_r"),
    ("right foot", "foot_r"),
    ("right toes", "ball_r"),
];

const BIPED: Naming = &[
    ("root", "Bip01"),
    ("hips", "Bip01 Pelvis"),
    ("spine", "Bip01 Spine"),
    ("chest", "Bip01 Spine1"),
    ("upper chest", "Bip01 Spine2"),
    ("neck", "Bip01 Neck"),
    ("head", "Bip01 Head"),
    ("head top", "Bip01 HeadNub"),
    ("left shoulder", "Bip01 L Clavicle"),
    ("left upper arm", "Bip01 L UpperArm"),
    ("left lower arm", "Bip01 L Forearm"),
    ("left hand", "Bip01 L Hand"),
    ("left thumb 1", "Bip01 L Finger0"),
    ("left thumb 2", "Bip01 L Finger01"),
    ("left thumb 3", "Bip01 L Finger02"),
    ("left thumb 4", "Bip01 L Finger0Nub"),
    ("left index 1", "Bip01 L Finger1"),
    ("left index 2", "Bip01 L Finger11"),
    ("left index 3", "Bip01 L Finger12"),
    ("left little 1", "Bip01 L Finger4"),
    ("left little 2", "Bip01 L Finger41"),
    ("left little 3", "Bip01 L Finger42"),
    ("right shoulder", "Bip01 R Clavicle"),
    ("right upper arm", "Bip01 R UpperArm"),
    ("right lower arm", "Bip01 R Forearm"),
    ("right hand", "Bip01 R Hand"),
    ("right index 1", "Bip01 R Finger1"),
    ("right index 2", "Bip01 R Finger11"),
    ("right index 3", "Bip01 R Finger12"),
    ("left upper leg", "Bip01 L Thigh"),
    ("left lower leg", "Bip01 L Calf"),
    ("left foot", "Bip01 L Foot"),
    ("left toes", "Bip01 L Toe0"),
    ("right upper leg", "Bip01 R Thigh"),
    ("right lower leg", "Bip01 R Calf"),
    ("right foot", "Bip01 R Foot"),
    ("right toes", "Bip01 R Toe0"),
];

const UNITY: Naming = &[
    ("hips", "Hips"),
    ("spine", "Spine"),
    ("chest", "Chest"),
    ("upper chest", "UpperChest"),
    ("neck", "Neck"),
    ("head", "Head"),
    ("left eye", "LeftEye"),
    ("right eye", "RightEye"),
    ("left shoulder", "LeftShoulder"),
    ("left upper arm", "LeftUpperArm"),
    ("left lower arm", "LeftLowerArm"),
    ("left hand", "LeftHand"),
    ("left thumb 1", "Left Thumb Proximal"),
    ("left thumb 2", "Left Thumb Intermediate"),
    ("left thumb 3", "Left Thumb Distal"),
    ("left index 1", "Left Index Proximal"),
    ("left index 2", "Left Index Intermediate"),
    ("left index 3", "Left Index Distal"),
    ("left little 1", "Left Little Proximal"),
    ("left little 2", "Left Little Intermediate"),
    ("left little 3", "Left Little Distal"),
    ("right shoulder", "RightShoulder"),
    ("right upper arm", "RightUpperArm"),
    ("right lower arm", "RightLowerArm"),
    ("right hand", "RightHand"),
    ("right index 1", "Right Index Proximal"),
    ("right index 2", "Right Index Intermediate"),
    ("right index 3", "Right Index Distal"),
    ("left upper leg", "LeftUpperLeg"),
    ("left lower leg", "LeftLowerLeg"),
    ("left foot", "LeftFoot"),
    ("left toes", "LeftToes"),
    ("right upper leg", "RightUpperLeg"),
    ("right lower leg", "RightLowerLeg"),
    ("right foot", "RightFoot"),
    ("right toes", "RightToes"),
];

const VROID: Naming = &[
    ("root", "Root"),
    ("hips", "J_Bip_C_Hips"),
    ("spine", "J_Bip_C_Spine"),
    ("chest", "J_Bip_C_Chest"),
    ("upper chest", "J_Bip_C_UpperChest"),
    ("neck", "J_Bip_C_Neck"),
    ("head", "J_Bip_C_Head"),
    ("left eye", "J_Adj_L_FaceEye"),
    ("right eye", "J_Adj_R_FaceEye"),
    ("left shoulder", "J_Bip_L_Shoulder"),
    ("left upper arm", "J_Bip_L_UpperArm"),
    ("left lower arm", "J_Bip_L_LowerArm"),
    ("left hand", "J_Bip_L_Hand"),
    ("left thumb 1", "J_Bip_L_Thumb1"),
    ("left thumb 2", "J_Bip_L_Thumb2"),
    ("left thumb 3", "J_Bip_L_Thumb3"),
    ("left index 1", "J_Bip_L_Index1"),
    ("left index 2", "J_Bip_L_Index2"),
    ("left index 3", "J_Bip_L_Index3"),
    ("left little 1", "J_Bip_L_Little1"),
    ("left little 2", "J_Bip_L_Little2"),
    ("left little 3", "J_Bip_L_Little3"),
    ("right shoulder", "J_Bip_R_Shoulder"),
    ("right upper arm", "J_Bip_R_UpperArm"),
    ("right lower arm", "J_Bip_R_LowerArm"),
    ("right hand", "J_Bip_R_Hand"),
    ("right index 1", "J_Bip_R_Index1"),
    ("right index 2", "J_Bip_R_Index2"),
    ("right index 3", "J_Bip_R_Index3"),
    ("left upper leg", "J_Bip_L_UpperLeg"),
    ("left lower leg", "J_Bip_L_LowerLeg"),
    ("left foot", "J_Bip_L_Foot"),
    ("left toes", "J_Bip_L_ToeBase"),
    ("right upper leg", "J_Bip_R_UpperLeg"),
    ("right lower leg", "J_Bip_R_LowerLeg"),
    ("right foot", "J_Bip_R_Foot"),
    ("right toes", "J_Bip_R_ToeBase"),
];

const CHARACTER_CREATOR: Naming = &[
    ("root", "CC_Base_BoneRoot"),
    ("hips", "CC_Base_Hip"),
    ("pelvis", "CC_Base_Pelvis"),
    ("spine", "CC_Base_Waist"),
    ("chest", "CC_Base_Spine01"),
    ("upper chest", "CC_Base_Spine02"),
    ("neck", "CC_Base_NeckTwist01"),
    ("head", "CC_Base_Head"),
    ("left eye", "CC_Base_L_Eye"),
    ("right eye", "CC_Base_R_Eye"),
    ("left shoulder", "CC_Base_L_Clavicle"),
    ("left upper arm", "CC_Base_L_Upperarm"),
    ("left arm twist", "CC_Base_L_UpperarmTwist01"),
    ("left lower arm", "CC_Base_L_Forearm"),
    ("left hand", "CC_Base_L_Hand"),
    ("left thumb 1", "CC_Base_L_Thumb1"),
    ("left thumb 2", "CC_Base_L_Thumb2"),
    ("left thumb 3", "CC_Base_L_Thumb3"),
    ("left index 1", "CC_Base_L_Index1"),
    ("left index 2", "CC_Base_L_Index2"),
    ("left index 3", "CC_Base_L_Index3"),
    ("left little 1", "CC_Base_L_Pinky1"),
    ("left little 2", "CC_Base_L_Pinky2"),
    ("left little 3", "CC_Base_L_Pinky3"),
    ("right shoulder", "CC_Base_R_Clavicle"),
    ("right upper arm", "CC_Base_R_Upperarm"),
    ("right arm twist", "CC_Base_R_UpperarmTwist01"),
    ("right lower arm", "CC_Base_R_Forearm"),
    ("right hand", "CC_Base_R_Hand"),
    ("right index 1", "CC_Base_R_Index1"),
    ("right index 2", "CC_Base_R_Index2"),
    ("right index 3", "CC_Base_R_Index3"),
    ("left upper leg", "CC_Base_L_Thigh"),
    ("left lower leg", "CC_Base_L_Calf"),
    ("left foot", "CC_Base_L_Foot"),
    ("left toes", "CC_Base_L_ToeBase"),
    ("right upper leg", "CC_Base_R_Thigh"),
    ("right lower leg", "CC_Base_R_Calf"),
    ("right foot", "CC_Base_R_Foot"),
    ("right toes", "CC_Base_R_ToeBase"),
];

const GENESIS: Naming = &[
    ("hips", "hip"),
    ("pelvis", "pelvis"),
    ("spine", "abdomenLower"),
    ("chest", "abdomenUpper"),
    ("upper chest", "chestUpper"),
    ("neck", "neckLower"),
    ("head", "head"),
    ("left eye", "lEye"),
    ("right eye", "rEye"),
    ("left shoulder", "lCollar"),
    ("left upper arm", "lShldrBend"),
    ("left arm twist", "lShldrTwist"),
    ("left lower arm", "lForearmBend"),
    ("left hand", "lHand"),
    ("left thumb 1", "lThumb1"),
    ("left thumb 2", "lThumb2"),
    ("left thumb 3", "lThumb3"),
    ("left index 1", "lIndex1"),
    ("left index 2", "lIndex2"),
    ("left index 3", "lIndex3"),
    ("left little 1", "lPinky1"),
    ("left little 2", "lPinky2"),
    ("left little 3", "lPinky3"),
    ("right shoulder", "rCollar"),
    ("right upper arm", "rShldrBend"),
    ("right arm twist", "rShldrTwist"),
    ("right lower arm", "rForearmBend"),
    ("right hand", "rHand"),
    ("right index 1", "rIndex1"),
    ("right index 2", "rIndex2"),
    ("right index 3", "rIndex3"),
    ("left upper leg", "lThighBend"),
    ("left lower leg", "lShin"),
    ("left foot", "lFoot"),
    ("left toes", "lToe"),
    ("right upper leg", "rThighBend"),
    ("right lower leg", "rShin"),
    ("right foot", "rFoot"),
    ("right toes", "rToe"),
];

const RIGIFY: Naming = &[
    ("root", "Armature"),
    ("hips", "DEF-spine"),
    ("spine", "DEF-spine.001"),
    ("chest", "DEF-spine.002"),
    ("upper chest", "DEF-spine.003"),
    ("neck", "DEF-spine.004"),
    ("head", "DEF-head"),
    ("left eye", "DEF-eye.L"),
    ("right eye", "DEF-eye.R"),
    ("left shoulder", "DEF-shoulder.L"),
    ("left upper arm", "DEF-upper_arm.L"),
    ("left lower arm", "DEF-forearm.L"),
    ("left hand", "DEF-hand.L"),
    ("left thumb 1", "DEF-thumb.01.L"),
    ("left thumb 2", "DEF-thumb.02.L"),
    ("left thumb 3", "DEF-thumb.03.L"),
    ("left index 1", "DEF-f_index.01.L"),
    ("left index 2", "DEF-f_index.02.L"),
    ("left index 3", "DEF-f_index.03.L"),
    ("left little 1", "DEF-f_pinky.01.L"),
    ("left little 2", "DEF-f_pinky.02.L"),
    ("left little 3", "DEF-f_pinky.03.L"),
    ("right shoulder", "DEF-shoulder.R"),
    ("right upper arm", "DEF-upper_arm.R"),
    ("right lower arm", "DEF-forearm.R"),
    ("right hand", "DEF-hand.R"),
    ("right index 1", "DEF-f_index.01.R"),
    ("right index 2", "DEF-f_index.02.R"),
    ("right index 3", "DEF-f_index.03.R"),
    ("left upper leg", "DEF-thigh.L"),
    ("left lower leg", "DEF-shin.L"),
    ("left foot", "DEF-foot.L"),
    ("left toes", "DEF-toe.L"),
    ("right upper leg", "DEF-thigh.R"),
    ("right lower leg", "DEF-shin.R"),
    ("right foot", "DEF-foot.R"),
    ("right toes", "DEF-toe.R"),
];

const MMD: Naming = &[
    ("root", "全ての親"),
    ("hips", "センター"),
    ("pelvis", "下半身"),
    ("spine", "上半身"),
    ("chest", "上半身2"),
    ("upper chest", "上半身3"),
    ("neck", "首"),
    ("head", "頭"),
    ("left eye", "左目"),
    ("right eye", "右目"),
    ("left shoulder", "左肩"),
    ("left upper arm", "左腕"),
    ("left arm twist", "左腕捩"),
    ("left lower arm", "左ひじ"),
    ("left hand", "左手首"),
    ("left thumb 1", "左親指０"),
    ("left thumb 2", "左親指１"),
    ("left thumb 3", "左親指２"),
    ("left thumb 4", "左親指先"),
    ("left index 1", "左人指１"),
    ("left index 2", "左人指２"),
    ("left index 3", "左人指３"),
    ("left index 4", "左人指先"),
    ("left little 1", "左小指１"),
    ("left little 2", "左小指２"),
    ("left little 3", "左小指３"),
    ("right shoulder", "右肩"),
    ("right upper arm", "右腕"),
    ("right arm twist", "右腕捩"),
    ("right lower arm", "右ひじ"),
    ("right hand", "右手首"),
    ("right index 1", "右人指１"),
    ("right index 2", "右人指２"),
    ("right index 3", "右人指３"),
    ("left upper leg", "左足"),
    ("left lower leg", "左ひざ"),
    ("left foot", "左足首"),
    ("left toes", "左足先EX"),
    ("right upper leg", "右足"),
    ("right lower leg", "右ひざ"),
    ("right foot", "右足首"),
    ("right toes", "右足先EX"),
];

fn skeleton(naming: Naming) -> (Model, Vec<(HumanBone, NodeId)>) {
    let name = |role: &str| {
        naming
            .iter()
            .find(|(named, _)| *named == role)
            .map(|(_, name)| *name)
    };
    let mut model = Model::default();
    let mut placed: Vec<(&str, NodeId, Vec3)> = Vec::new();
    let mut expected = Vec::new();
    for joint in LAYOUT {
        let Some(named) = name(joint.role) else {
            continue;
        };
        let mut above = joint.parent;
        let parent = loop {
            match above {
                Some(role) if name(role).is_none() => {
                    above = LAYOUT
                        .iter()
                        .find(|candidate| candidate.role == role)
                        .and_then(|candidate| candidate.parent);
                }
                Some(role) => {
                    break placed
                        .iter()
                        .find(|(placed_role, _, _)| *placed_role == role)
                        .map(|(_, node, at)| (*node, *at));
                }
                None => break None,
            }
        };
        let at = Vec3::from(joint.at);
        let node = NodeId(model.nodes.len() as u32);
        let mut created = Node::new(named).transform(Transform::from_translation(
            at - parent.map_or(Vec3::ZERO, |(_, above)| above),
        ));
        created.parent = parent.map(|(node, _)| node);
        if let Some((parent, _)) = parent {
            model.nodes[parent.index()].children.push(node);
        }
        model.nodes.push(created);
        placed.push((joint.role, node, at));
        if let Some(bone) = joint.bone {
            expected.push((bone, node));
        }
    }
    (model, expected)
}

#[test]
fn rig_conventions_map_by_name() {
    for (convention, naming) in [
        ("Mixamo", MIXAMO),
        ("Unreal", UNREAL),
        ("3ds Max Biped", BIPED),
        ("Unity", UNITY),
        ("VRoid", VROID),
        ("Character Creator", CHARACTER_CREATOR),
        ("Genesis", GENESIS),
        ("Rigify", RIGIFY),
        ("MMD", MMD),
    ] {
        let (model, expected) = skeleton(naming);
        let anatomy = Anatomy::recognize(&model);
        let humanoid = anatomy.humanoid();
        assert_eq!(humanoid.missing().count(), 0, "{convention}: {humanoid:?}");
        for (bone, node) in expected {
            assert_eq!(
                humanoid
                    .bone(bone)
                    .map(|found| model.node(found).name.clone()),
                Some(model.node(node).name.clone()),
                "{convention}: {bone}"
            );
        }
        assert_eq!(
            humanoid.len(),
            LAYOUT
                .iter()
                .filter(|joint| joint.bone.is_some()
                    && naming.iter().any(|(role, _)| *role == joint.role))
                .count(),
            "{convention}: {humanoid:?}"
        );
        assert!(
            anatomy.forward().z > 0.9,
            "{convention}: {:?}",
            anatomy.forward()
        );
    }
}

#[test]
fn unknown_names_leave_skeleton_unmapped() {
    let (mut model, _) = skeleton(MIXAMO);
    for (index, node) in model.nodes.iter_mut().enumerate() {
        node.name = format!("node {index}");
    }
    assert!(Anatomy::recognize(&model).humanoid().missing().count() > 0);
}

#[test]
fn standard_mmd_skeleton_maps_by_name() {
    let model = rikka_mmd::standard_skeleton();
    let anatomy = Anatomy::recognize(&model);
    let named = |bone: HumanBone| anatomy.node(bone).map(|node| model.node(node).name.clone());
    assert_eq!(anatomy.humanoid().missing().count(), 0);
    for (bone, name) in [
        (B::Hips, "腰"),
        (B::Spine, "上半身"),
        (B::Chest, "上半身2"),
        (B::Neck, "首"),
        (B::Head, "頭"),
        (B::LeftEye, "左目"),
        (B::LeftShoulder, "左肩"),
        (B::LeftUpperArm, "左腕"),
        (B::RightHand, "右手首"),
        (B::LeftThumbMetacarpal, "左親指０"),
        (B::RightLittleDistal, "右小指３"),
        (B::LeftUpperLeg, "左足"),
        (B::RightFoot, "右足首"),
    ] {
        assert_eq!(named(bone).as_deref(), Some(name), "{bone}");
    }
    let twist = anatomy
        .twist(B::LeftUpperArm)
        .map(|node| model.node(node).name.clone());
    assert_eq!(twist.as_deref(), Some("左腕捩"));
}

#[test]
fn knight_maps_wrist_before_hand() {
    let model = Import::named_bytes(
        "knight.fbx",
        KNIGHT.to_vec(),
        Arc::new(Registry::new().with_importer(FbxImporter)),
    )
    .run()
    .expect("the knight reads");
    let anatomy = Anatomy::recognize(&model);
    let named = |bone: HumanBone| anatomy.node(bone).map(|node| model.node(node).name.clone());
    assert_eq!(anatomy.humanoid().missing().count(), 0);
    for (bone, name) in [
        (B::Hips, "hips"),
        (B::Spine, "spine"),
        (B::Head, "head"),
        (B::LeftUpperArm, "upperarm.l"),
        (B::LeftLowerArm, "lowerarm.l"),
        (B::LeftHand, "wrist.l"),
        (B::RightHand, "wrist.r"),
        (B::LeftUpperLeg, "upperleg.l"),
        (B::RightFoot, "foot.r"),
        (B::LeftToes, "toes.l"),
    ] {
        assert_eq!(named(bone).as_deref(), Some(name), "{bone}");
    }
}
