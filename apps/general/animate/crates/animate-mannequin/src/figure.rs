use rikka_model_core::glam::{Mat4, Vec3, Vec4};
use rikka_model_core::{
    Material, MaterialId, Mesh, Model, ModelBuilder, Node, NodeId, Pbr, Shading, Skin, Submesh,
    Transform, Vertices,
};

use crate::shell::{Capsule, Ellipsoid, Finish, Part, Shell};
use crate::skeleton::{
    CHEST, HEAD, HIPS, Joint, NECK, SPINE, Side, TOE_TIP, UPPER_CHEST, joint_at, joints,
};

const NAME: &str = "Mannequin";
const SHELL_COLOR: Vec4 = Vec4::new(0.72, 0.71, 0.69, 1.0);
const JOINT_COLOR: Vec4 = Vec4::new(0.42, 0.42, 0.43, 1.0);
const SHELL_ROUGHNESS: f32 = 0.55;
const JOINT_ROUGHNESS: f32 = 0.4;

struct Bones<'a> {
    joints: &'a [Joint],
}

impl Bones<'_> {
    fn index(&self, name: &str) -> u16 {
        self.joints
            .iter()
            .position(|joint| joint.name == name)
            .expect("the mannequin names only its own joints") as u16
    }
}

fn limb(from: &str, to: Vec3, radius: f32, end_radius: f32) -> Capsule {
    Capsule {
        from: joint_at(from),
        to,
        radius,
        end_radius,
    }
}

fn trunk(shell: &mut Shell, bones: &Bones<'_>) {
    let pieces = [
        (
            HEAD,
            Vec3::new(0.0, 1.665, 0.012),
            Vec3::new(0.083, 0.112, 0.098),
        ),
        (
            UPPER_CHEST,
            Vec3::new(0.0, 1.335, -0.005),
            Vec3::new(0.155, 0.15, 0.1),
        ),
        (
            CHEST,
            Vec3::new(0.0, 1.21, 0.0),
            Vec3::new(0.142, 0.115, 0.09),
        ),
        (
            SPINE,
            Vec3::new(0.0, 1.095, 0.0),
            Vec3::new(0.135, 0.105, 0.086),
        ),
        (
            HIPS,
            Vec3::new(0.0, 0.96, -0.005),
            Vec3::new(0.16, 0.112, 0.1),
        ),
    ];
    for (bone, center, axes) in pieces {
        shell.ellipsoid(Ellipsoid { center, axes }, bones.index(bone), Finish::Shell);
    }
    shell.capsule(
        limb(
            NECK,
            joint_at(HEAD) + Vec3::new(0.0, 0.02, 0.0),
            0.043,
            0.04,
        ),
        bones.index(NECK),
        Finish::Shell,
    );
}

fn arm(shell: &mut Shell, bones: &Bones<'_>, side: Side) {
    let bone = |role: &str| side.bone(role);
    let at = |role: &str| joint_at(&bone(role));
    shell.capsule(
        limb(&bone("Shoulder"), at("UpperArm"), 0.034, 0.04),
        bones.index(&bone("Shoulder")),
        Finish::Shell,
    );
    shell.ball(at("UpperArm"), 0.05, bones.index(&bone("UpperArm")));
    shell.capsule(
        limb(&bone("UpperArm"), at("LowerArm"), 0.044, 0.036),
        bones.index(&bone("UpperArm")),
        Finish::Shell,
    );
    shell.ball(at("LowerArm"), 0.036, bones.index(&bone("LowerArm")));
    shell.capsule(
        limb(&bone("LowerArm"), at("Hand"), 0.035, 0.026),
        bones.index(&bone("LowerArm")),
        Finish::Shell,
    );
    shell.ball(at("Hand"), 0.026, bones.index(&bone("Hand")));
    shell.ellipsoid(
        Ellipsoid {
            center: side.place(Vec3::new(0.738, 1.43, -0.017)),
            axes: Vec3::new(0.05, 0.017, 0.043),
        },
        bones.index(&bone("Hand")),
        Finish::Shell,
    );
    for finger in side.fingers() {
        let ends = [finger.joints[1], finger.joints[2], finger.tip];
        for (index, (from, to)) in finger.joints.into_iter().zip(ends).enumerate() {
            let taper = 1.0 - 0.08 * (index as f32 + 1.0);
            shell.capsule(
                Capsule {
                    from,
                    to,
                    radius: finger.radius * (taper + 0.08),
                    end_radius: finger.radius * taper,
                },
                bones.index(&side.finger_bone(finger.name, index)),
                Finish::Shell,
            );
        }
    }
}

fn leg(shell: &mut Shell, bones: &Bones<'_>, side: Side) {
    let bone = |role: &str| side.bone(role);
    let at = |role: &str| joint_at(&bone(role));
    shell.ball(at("UpperLeg"), 0.07, bones.index(&bone("UpperLeg")));
    shell.capsule(
        limb(&bone("UpperLeg"), at("LowerLeg"), 0.074, 0.05),
        bones.index(&bone("UpperLeg")),
        Finish::Shell,
    );
    shell.ball(at("LowerLeg"), 0.05, bones.index(&bone("LowerLeg")));
    shell.capsule(
        limb(&bone("LowerLeg"), at("Foot"), 0.048, 0.032),
        bones.index(&bone("LowerLeg")),
        Finish::Shell,
    );
    shell.ball(at("Foot"), 0.033, bones.index(&bone("Foot")));
    shell.ellipsoid(
        Ellipsoid {
            center: side.place(Vec3::new(0.09, 0.042, 0.035)),
            axes: Vec3::new(0.044, 0.038, 0.085),
        },
        bones.index(&bone("Foot")),
        Finish::Shell,
    );
    let toes = at("ToeBase");
    shell.ellipsoid(
        Ellipsoid {
            center: (toes + side.place(TOE_TIP)) * 0.5,
            axes: Vec3::new(0.04, 0.023, 0.045),
        },
        bones.index(&bone("ToeBase")),
        Finish::Shell,
    );
}

fn material(name: &str, color: Vec4, roughness: f32) -> Material {
    Material::new(name)
        .base_color(color)
        .shading(Shading::Pbr(Pbr {
            metallic: 0.0,
            roughness,
            ..Pbr::default()
        }))
}

fn mesh(shell: Shell, shell_material: MaterialId, joint_material: MaterialId) -> Mesh {
    let mut vertices = Vertices::default();
    let mut indices = Vec::new();
    let mut submeshes = Vec::new();
    for (name, part, material) in [
        ("shell", shell.shell, shell_material),
        ("joints", shell.joint, joint_material),
    ] {
        let Part {
            positions,
            normals,
            joints,
            indices: local,
        } = part;
        let base = vertices.positions.len() as u32;
        let first_index = indices.len() as u32;
        indices.extend(local.iter().map(|index| base + index));
        vertices.positions.extend(positions);
        vertices.normals.extend(normals);
        vertices
            .joints
            .extend(joints.into_iter().map(|joint| [joint, 0, 0, 0]));
        submeshes.push(Submesh {
            name: name.to_owned(),
            first_index,
            index_count: indices.len() as u32 - first_index,
            material: Some(material),
            bounds: Default::default(),
        });
    }
    vertices.weights = vec![Vec4::X; vertices.positions.len()];
    Mesh {
        vertices,
        indices,
        submeshes,
        ..Mesh::new(NAME)
    }
}

pub(crate) fn assemble() -> Model {
    let joints = joints();
    let bones = Bones { joints: &joints };
    let mut shell = Shell::default();
    trunk(&mut shell, &bones);
    for side in Side::BOTH {
        arm(&mut shell, &bones, side);
        leg(&mut shell, &bones, side);
    }

    let mut builder = ModelBuilder::new().generator(NAME);
    let mut nodes: Vec<NodeId> = Vec::with_capacity(joints.len());
    for joint in &joints {
        let parent = joint.parent.as_deref().map(|parent| {
            let index = bones.index(parent) as usize;
            (nodes[index], joints[index].at)
        });
        let mut node = Node::new(joint.name.clone()).transform(Transform::from_translation(
            joint.at - parent.map_or(Vec3::ZERO, |(_, at)| at),
        ));
        node.parent = parent.map(|(id, _)| id);
        nodes.push(builder.add_node(node));
    }
    let shell_material = builder.add_material(material("Shell", SHELL_COLOR, SHELL_ROUGHNESS));
    let joint_material = builder.add_material(material("Joint", JOINT_COLOR, JOINT_ROUGHNESS));
    let mesh = builder.add_mesh(mesh(shell, shell_material, joint_material));
    let skin = builder.add_skin(Skin {
        joints: nodes.clone(),
        inverse_bind: joints
            .iter()
            .map(|joint| Mat4::from_translation(-joint.at))
            .collect(),
        skeleton: nodes.first().copied(),
        ..Skin::new(NAME)
    });
    builder.add_node(Node::new(NAME).mesh(mesh).skin(skin));
    builder.finish().expect("the mannequin is a valid model")
}
