use rikka_model_core::glam::{Mat4, Vec3};

use crate::document::{Node, Value};
use crate::euler::{self, camera_facing, light_facing};
use crate::write::geometry::Meshes;
use crate::write::material::MaterialIds;
use crate::write::{Composer, doubles, integer, named, number, prop, vector};
use rikka_model_core::EulerOrder;
use rikka_model_core::LightKind;
use rikka_model_core::Model;
use rikka_model_core::NodeId;

pub(crate) struct NodeIds {
    nodes: Vec<i64>,
}

impl NodeIds {
    pub(crate) fn node(&self, id: NodeId) -> i64 {
        self.nodes[id.index()]
    }
}

pub(crate) fn write(
    model: &Model,
    composer: &mut Composer,
    meshes: &Meshes,
    painted: &MaterialIds,
) -> NodeIds {
    let joints = joints(model);
    let node_ids = NodeIds {
        nodes: model.nodes.iter().map(|_| composer.id()).collect(),
    };

    for (index, node) in model.nodes.iter().enumerate() {
        let id = node_ids.nodes[index];
        let kind = match (node.mesh, node.camera, node.light, joints[index]) {
            (Some(_), ..) => "Mesh",
            (_, Some(_), ..) => "Camera",
            (_, _, Some(_), _) => "Light",
            (.., true) => "LimbNode",
            _ => "Null",
        };
        composer.add("Model", model_node(&node.name, id, node, kind));

        match node.parent {
            Some(parent) => composer.connect(id, node_ids.node(parent)),
            None => composer.connect(id, 0),
        }

        if let Some(mesh) = node.mesh
            && let Some(record) = meshes.get(mesh)
        {
            composer.connect(record.geometry, id);
            for material in &record.materials {
                if let Some(material_id) = painted.material(*material) {
                    composer.connect(material_id, id);
                }
            }
        }
        if let Some(camera) = node.camera {
            let attribute = composer.id();
            composer.add("NodeAttribute", camera_attribute(model, camera, attribute));
            composer.connect(attribute, id);
        }
        if let Some(light) = node.light {
            let attribute = composer.id();
            composer.add("NodeAttribute", light_attribute(model, light, attribute));
            composer.connect(attribute, id);
        }
    }

    if let Some(pose) = bind_pose(model, &node_ids, composer) {
        composer.add("Pose", pose);
    }
    node_ids
}

fn model_node(name: &str, id: i64, node: &rikka_model_core::Node, kind: &str) -> Node {
    let mut rotation = node.transform.rotation;
    if node.camera.is_some() {
        rotation *= camera_facing().conjugate();
    }
    if node.light.is_some() {
        rotation *= light_facing().conjugate();
    }

    Node::new("Model")
        .with_i64(id)
        .with_text(named(name, "Model"))
        .with_text(kind)
        .adding(Node::new("Version").with_i32(232))
        .adding(
            Node::new("Properties70")
                .adding(prop("InheritType", "enum", "", "").with_i32(0))
                .adding(integer("DefaultAttributeIndex", 0))
                .adding(vector(
                    "Lcl Translation",
                    "Lcl Translation",
                    node.transform.translation,
                ))
                .adding(vector(
                    "Lcl Rotation",
                    "Lcl Rotation",
                    euler::angles(EulerOrder::Xyz, rotation),
                ))
                .adding(vector("Lcl Scaling", "Lcl Scaling", node.transform.scale)),
        )
        .adding(Node::new("Shading").with(Value::Bool(true)))
        .adding(Node::new("Culling").with_text("CullingOff"))
}

const FAR_PLANE: f32 = 1000.0;
fn camera_attribute(model: &Model, camera_id: rikka_model_core::CameraId, id: i64) -> Node {
    let camera = model.camera(camera_id);
    let mut props = Node::new("Properties70")
        .adding(
            prop("Position", "Vector", "", "A")
                .with_f64(0.0)
                .with_f64(0.0)
                .with_f64(0.0),
        )
        .adding(vector("UpVector", "Vector", Vec3::Y))
        .adding(vector("InterestPosition", "Vector", Vec3::X));

    match camera.projection {
        rikka_model_core::Projection::Perspective {
            y_fov,
            aspect,
            near,
            far,
        } => {
            props.add(prop("CameraProjectionType", "enum", "", "").with_i32(0));
            props.add(number("FieldOfViewY", f64::from(y_fov.to_degrees())));
            props.add(number("FieldOfView", f64::from(y_fov.to_degrees())));
            props.add(number(
                "AspectWidth",
                f64::from(aspect.unwrap_or(1.0)) * 1000.0,
            ));
            props.add(number("AspectHeight", 1000.0));
            props.add(number("NearPlane", f64::from(near)));
            props.add(number(
                "FarPlane",
                f64::from(far.unwrap_or(FAR_PLANE / model.metadata.unit_scale)),
            ));
        }
        rikka_model_core::Projection::Orthographic {
            height, near, far, ..
        } => {
            props.add(prop("CameraProjectionType", "enum", "", "").with_i32(1));
            props.add(number(
                "OrthoZoom",
                f64::from(height * model.metadata.unit_scale),
            ));
            props.add(number("NearPlane", f64::from(near)));
            props.add(number("FarPlane", f64::from(far)));
        }
    }

    Node::new("NodeAttribute")
        .with_i64(id)
        .with_text(named(&camera.name, "NodeAttribute"))
        .with_text("Camera")
        .adding(Node::new("TypeFlags").with_text("Camera"))
        .adding(Node::new("GeometryVersion").with_i32(124))
        .adding(props)
}

fn light_attribute(model: &Model, light_id: rikka_model_core::LightId, id: i64) -> Node {
    let light = model.light(light_id);
    let mut props = Node::new("Properties70")
        .adding(vector("Color", "Color", light.color))
        .adding(number("Intensity", f64::from(light.intensity) * 100.0));

    let range = match light.kind {
        LightKind::Directional => {
            props.add(prop("LightType", "enum", "", "").with_i32(1));
            None
        }
        LightKind::Point { range } => {
            props.add(prop("LightType", "enum", "", "").with_i32(0));
            Some(range)
        }
        LightKind::Spot {
            range,
            inner_angle,
            outer_angle,
        } => {
            props.add(prop("LightType", "enum", "", "").with_i32(2));
            props.add(number(
                "InnerAngle",
                f64::from(inner_angle.to_degrees()) * 2.0,
            ));
            props.add(number(
                "OuterAngle",
                f64::from(outer_angle.to_degrees()) * 2.0,
            ));
            Some(range)
        }
    };
    if let Some(range) = range.filter(|range| range.is_finite()) {
        props.add(prop("EnableFarAttenuation", "bool", "", "").with_i32(1));
        props.add(number("FarAttenuationEnd", f64::from(range)));
    }

    Node::new("NodeAttribute")
        .with_i64(id)
        .with_text(named(&light.name, "NodeAttribute"))
        .with_text("Light")
        .adding(Node::new("TypeFlags").with_text("Light"))
        .adding(Node::new("GeometryVersion").with_i32(124))
        .adding(props)
}

fn bind_pose(model: &Model, node_ids: &NodeIds, composer: &mut Composer) -> Option<Node> {
    if model.skins.is_empty() {
        return None;
    }
    let id = composer.id();
    let mut node = Node::new("Pose")
        .with_i64(id)
        .with_text(named("BindPose", "Pose"))
        .with_text("BindPose")
        .adding(Node::new("Type").with_text("BindPose"))
        .adding(Node::new("Version").with_i32(100));

    let mut posed: Vec<(NodeId, Mat4)> = Vec::new();
    for skin in &model.skins {
        for (joint, bind) in skin.joints.iter().zip(&skin.inverse_bind) {
            if !posed.iter().any(|(posed_joint, _)| posed_joint == joint) {
                posed.push((*joint, bind.inverse()));
            }
        }
    }
    node.add(Node::new("NbPoseNodes").with_i32(posed.len() as i32));
    for (joint, matrix) in posed {
        node.add(
            Node::new("PoseNode")
                .adding(Node::new("Node").with_i64(node_ids.node(joint)))
                .adding(Node::new("Matrix").with(doubles(matrix.to_cols_array()))),
        );
    }
    Some(node)
}

fn joints(model: &Model) -> Vec<bool> {
    let mut is_joint = vec![false; model.nodes.len()];
    for skin in &model.skins {
        for joint in &skin.joints {
            is_joint[joint.index()] = true;
        }
    }
    is_joint
}
