use rikka_model_core::glam::{Mat4, Quat, Vec3};

use crate::document::{self, CameraKind, Document, LightKind as DescribedKind};
use rikka_model_core::ImportCtx;
use rikka_model_core::MeshId;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Result;
use rikka_model_core::SkinId;
use rikka_model_core::Transform;
use rikka_model_core::Warning;
use rikka_model_core::{Camera, CameraId, Projection};
use rikka_model_core::{Light, LightId, LightKind};
use rikka_model_core::{Node, NodeId};

pub(crate) fn read(
    document: &Document,
    meshes: &[Option<MeshId>],
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<()> {
    read_cameras(document, builder);
    read_lights(document, builder);

    for (index, described) in document.nodes.iter().enumerate() {
        let name = described
            .name
            .clone()
            .unwrap_or_else(|| format!("node{index}"));
        builder.add_node(Node::new(name).transform(placement(described)));
    }

    let mut claimed = vec![false; document.nodes.len()];
    for (index, described) in document.nodes.iter().enumerate() {
        ctx.checkpoint()?;
        let parent = NodeId(index as u32);
        for child in &described.children {
            let Some(claim) = claimed.get_mut(*child as usize) else {
                ctx.warn(Warning::Repaired(format!(
                    "node {index} references missing child {child}"
                )));
                continue;
            };
            if std::mem::replace(claim, true) {
                ctx.warn(Warning::Repaired(format!(
                    "node {child} has several parents, keeping the first"
                )));
                continue;
            }
            builder.attach(parent, NodeId(*child));
        }
    }

    let known_lights = lights(document).len();
    for (index, described) in document.nodes.iter().enumerate() {
        let mesh = described
            .mesh
            .and_then(|mesh| meshes.get(mesh as usize).copied().flatten());
        let node = &mut builder.model_mut().nodes[index];
        node.mesh = mesh;
        node.skin = described
            .skin
            .filter(|skin| (*skin as usize) < document.skins.len())
            .map(SkinId);
        node.camera = described
            .camera
            .filter(|camera| (*camera as usize) < document.cameras.len())
            .map(CameraId);
        node.light = described
            .extensions
            .lights_punctual
            .as_ref()
            .map(|lights| lights.light)
            .filter(|light| (*light as usize) < known_lights)
            .map(LightId);
        node.morph_weights = morph_weights(document, described);
    }
    Ok(())
}

fn placement(described: &document::Node) -> Transform {
    match described.matrix {
        Some(matrix) => Transform::from_matrix(Mat4::from_cols_array(&matrix)),
        None => Transform {
            translation: Vec3::from_array(described.translation.unwrap_or([0.0; 3])),
            rotation: Quat::from_array(described.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]))
                .normalize(),
            scale: Vec3::from_array(described.scale.unwrap_or([1.0; 3])),
        },
    }
}

fn morph_weights(document: &Document, described: &document::Node) -> Vec<f32> {
    if !described.weights.is_empty() {
        return described.weights.clone();
    }
    described
        .mesh
        .and_then(|mesh| document.meshes.get(mesh as usize))
        .map(|mesh| mesh.weights.clone())
        .unwrap_or_default()
}

fn read_cameras(document: &Document, builder: &mut ModelBuilder) {
    for (index, described) in document.cameras.iter().enumerate() {
        let name = described
            .name
            .clone()
            .unwrap_or_else(|| format!("camera{index}"));
        builder.add_camera(Camera::new(name).projection(projection(described)));
    }
}

fn projection(described: &document::Camera) -> Projection {
    match described.kind {
        CameraKind::Orthographic => {
            let orthographic = described.orthographic.clone().unwrap_or_default();
            Projection::Orthographic {
                width: orthographic.xmag * 2.0,
                height: orthographic.ymag * 2.0,
                near: orthographic.znear,
                far: orthographic.zfar,
            }
        }
        CameraKind::Perspective => {
            let perspective = described.perspective.clone().unwrap_or_default();
            Projection::Perspective {
                y_fov: perspective.yfov,
                aspect: perspective.aspect_ratio,
                near: perspective.znear,
                far: perspective.zfar,
            }
        }
    }
}

fn read_lights(document: &Document, builder: &mut ModelBuilder) {
    for (index, described) in lights(document).iter().enumerate() {
        let name = described
            .name
            .clone()
            .unwrap_or_else(|| format!("light{index}"));
        let range = described.range.unwrap_or(f32::INFINITY);
        let spot = described.spot.clone().unwrap_or_default();
        let kind = match described.kind {
            DescribedKind::Directional => LightKind::Directional,
            DescribedKind::Point => LightKind::Point { range },
            DescribedKind::Spot => LightKind::Spot {
                range,
                inner_angle: spot.inner_cone_angle.unwrap_or(0.0),
                outer_angle: spot.outer_cone_angle.unwrap_or(std::f32::consts::FRAC_PI_4),
            },
        };
        builder.add_light(
            Light::new(name, kind)
                .color(Vec3::from_array(described.color.unwrap_or([1.0; 3])))
                .intensity(described.intensity.unwrap_or(1.0)),
        );
    }
}

fn lights(document: &Document) -> &[document::Light] {
    document
        .extensions
        .lights_punctual
        .as_ref()
        .map_or(&[], |lights| lights.lights.as_slice())
}
