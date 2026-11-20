use crate::document::{
    Camera as DescribedCamera, CameraKind, Document, DocumentExtensions, Index,
    Light as DescribedLight, LightKind as DescribedKind, LightsPunctual, Node as DescribedNode,
    NodeExtensions, NodeLight, Orthographic, Perspective, Scene, Spot,
};
use rikka_model_core::LightKind;
use rikka_model_core::Model;
use rikka_model_core::Projection;
use rikka_model_core::Transform;

const LIGHTS: &str = "KHR_lights_punctual";

pub(crate) fn write(
    model: &Model,
    document: &mut Document,
    meshes: &[Option<Index>],
    skins: &[Option<Index>],
) {
    document.cameras = model
        .cameras
        .iter()
        .map(|camera| DescribedCamera {
            kind: match camera.projection {
                Projection::Perspective { .. } => CameraKind::Perspective,
                Projection::Orthographic { .. } => CameraKind::Orthographic,
            },
            perspective: match camera.projection {
                Projection::Perspective {
                    y_fov,
                    aspect,
                    near,
                    far,
                } => Some(Perspective {
                    yfov: y_fov,
                    znear: near,
                    aspect_ratio: aspect,
                    zfar: far,
                }),
                Projection::Orthographic { .. } => None,
            },
            orthographic: match camera.projection {
                Projection::Orthographic {
                    width,
                    height,
                    near,
                    far,
                } => Some(Orthographic {
                    xmag: width * 0.5,
                    ymag: height * 0.5,
                    znear: near,
                    zfar: far,
                }),
                Projection::Perspective { .. } => None,
            },
            name: Some(camera.name.clone()).filter(|name| !name.is_empty()),
        })
        .collect();

    if !model.lights.is_empty() {
        document.extensions = DocumentExtensions {
            lights_punctual: Some(LightsPunctual {
                lights: model.lights.iter().map(described_light).collect(),
            }),
            ..DocumentExtensions::default()
        };
        document.extensions_used.push(LIGHTS.to_owned());
    }

    document.nodes = model
        .nodes
        .iter()
        .map(|node| described_node(node, meshes, skins))
        .collect();
    document.scenes = vec![Scene {
        nodes: model.roots.iter().map(|root| root.0).collect(),
        name: None,
    }];
    document.scene = Some(0);
}

fn described_node(
    node: &rikka_model_core::Node,
    meshes: &[Option<Index>],
    skins: &[Option<Index>],
) -> DescribedNode {
    let placement = node.transform;
    DescribedNode {
        children: node.children.iter().map(|child| child.0).collect(),
        matrix: None,
        translation: (placement.translation != Transform::IDENTITY.translation)
            .then(|| placement.translation.to_array()),
        rotation: (placement.rotation != Transform::IDENTITY.rotation)
            .then(|| placement.rotation.to_array()),
        scale: (placement.scale != Transform::IDENTITY.scale).then(|| placement.scale.to_array()),
        mesh: node.mesh.and_then(|mesh| meshes[mesh.index()]),
        skin: node.skin.and_then(|skin| skins[skin.index()]),
        camera: node.camera.map(|camera| camera.0),
        weights: node.morph_weights.clone(),
        name: Some(node.name.clone()).filter(|name| !name.is_empty()),
        extensions: NodeExtensions {
            lights_punctual: node.light.map(|light| NodeLight { light: light.0 }),
            ..NodeExtensions::default()
        },
    }
}

fn described_light(light: &rikka_model_core::Light) -> DescribedLight {
    let range = match light.kind {
        LightKind::Directional => None,
        LightKind::Point { range } | LightKind::Spot { range, .. } => {
            range.is_finite().then_some(range)
        }
    };
    DescribedLight {
        kind: match light.kind {
            LightKind::Directional => DescribedKind::Directional,
            LightKind::Point { .. } => DescribedKind::Point,
            LightKind::Spot { .. } => DescribedKind::Spot,
        },
        color: Some(light.color.to_array()),
        intensity: Some(light.intensity),
        range,
        spot: match light.kind {
            LightKind::Spot {
                inner_angle,
                outer_angle,
                ..
            } => Some(Spot {
                inner_cone_angle: Some(inner_angle),
                outer_cone_angle: Some(outer_angle),
            }),
            _ => None,
        },
        name: Some(light.name.clone()).filter(|name| !name.is_empty()),
    }
}
