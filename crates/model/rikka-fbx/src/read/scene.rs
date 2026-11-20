use rikka_model_core::glam::{Mat4, Vec3};
use rustc_hash::FxHashMap;

use crate::euler::{camera_facing, light_facing};
use crate::read::graph::{Graph, Object};
use crate::read::props::Props;
use crate::read::transform::{Inherit, Pipeline, geometry};
use rikka_model_core::ImportCtx;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Result;
use rikka_model_core::Transform;
use rikka_model_core::Warning;
use rikka_model_core::{Camera, Projection};
use rikka_model_core::{Light, LightKind};
use rikka_model_core::{Node, NodeId};

pub(crate) struct Scene {
    pub nodes: FxHashMap<i64, NodeId>,
    pub carriers: FxHashMap<i64, NodeId>,
    pub world: FxHashMap<i64, Mat4>,
    pub pipelines: FxHashMap<i64, Pipeline>,
    pub statics: FxHashMap<i64, (Vec3, Vec3, Vec3)>,
    pub models: Vec<i64>,
}

pub(crate) fn read(
    graph: &Graph<'_>,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<Scene> {
    let models: Vec<&Object<'_>> = graph
        .objects()
        .filter(|object| object.kind == "Model")
        .collect();
    let parents = parents(graph, &models);
    let ordered = ordered(&models, &parents);

    let mut scene = Scene {
        nodes: FxHashMap::default(),
        carriers: FxHashMap::default(),
        world: FxHashMap::default(),
        pipelines: FxHashMap::default(),
        statics: FxHashMap::default(),
        models: ordered.iter().map(|object| object.id).collect(),
    };
    let mut inherited: FxHashMap<i64, Vec3> = FxHashMap::default();
    let mut warned = false;

    for object in &ordered {
        ctx.checkpoint()?;
        let pipeline = Pipeline::read(&object.props);
        let mut local = pipeline.local(&object.props);
        let parent = parents.get(&object.id).copied();
        let handed = parent
            .and_then(|parent| inherited.get(&parent).copied())
            .unwrap_or(Vec3::ONE);

        if pipeline.inherit == Inherit::RotationOnly {
            local.scale /= handed;
            inherited.insert(object.id, pipeline.local(&object.props).scale);
            if !warned {
                warned = true;
                ctx.warn(Warning::Repaired(
                    "non-inherited scale is baked into the final scale".to_owned(),
                ));
            }
        } else {
            inherited.insert(object.id, handed * local.scale);
        }

        let node = builder.add_node(Node {
            parent: parent.and_then(|parent| scene.nodes.get(&parent).copied()),
            ..Node::new(object.name.clone()).transform(local)
        });
        scene.nodes.insert(object.id, node);
        scene.pipelines.insert(object.id, pipeline);
        scene.statics.insert(
            object.id,
            (
                object.props.vec3("Lcl Translation", Vec3::ZERO),
                object.props.vec3("Lcl Rotation", Vec3::ZERO),
                object.props.vec3("Lcl Scaling", Vec3::ONE),
            ),
        );

        let world = parent
            .and_then(|parent| scene.world.get(&parent).copied())
            .unwrap_or(Mat4::IDENTITY)
            * local.to_matrix();
        scene.world.insert(object.id, world);

        let placement = geometry(&object.props);
        let carrier = match placement == Transform::IDENTITY {
            true => node,
            false => builder.add_node(Node {
                parent: Some(node),
                ..Node::new(object.name.clone()).transform(placement)
            }),
        };
        scene.carriers.insert(object.id, carrier);

        attach(object, graph, node, builder);
    }
    Ok(scene)
}

fn attach(object: &Object<'_>, graph: &Graph<'_>, node: NodeId, builder: &mut ModelBuilder) {
    for (kind, props) in attributes(object, graph) {
        match kind {
            "Camera" => {
                let unit_scale = builder.model_mut().metadata.unit_scale;
                let camera = builder.add_camera(camera(&object.name, props, unit_scale));
                builder.add_node(Node {
                    parent: Some(node),
                    camera: Some(camera),
                    ..Node::new(object.name.clone()).transform(Transform {
                        rotation: camera_facing(),
                        ..Transform::IDENTITY
                    })
                });
            }
            "Light" => {
                let light = builder.add_light(light(&object.name, props));
                builder.add_node(Node {
                    parent: Some(node),
                    light: Some(light),
                    ..Node::new(object.name.clone()).transform(Transform {
                        rotation: light_facing(),
                        ..Transform::IDENTITY
                    })
                });
            }
            _ => {}
        }
    }
}

fn attributes<'a>(object: &'a Object<'_>, graph: &'a Graph<'_>) -> Vec<(&'a str, &'a Props)> {
    let attributes: Vec<(&str, &Props)> = graph
        .sources(object.id)
        .filter(|(source, _)| source.kind == "NodeAttribute")
        .map(|(source, _)| (source.sub_kind.as_str(), &source.props))
        .collect();
    match attributes.is_empty() {
        false => attributes,
        true => vec![(object.sub_kind.as_str(), &object.props)],
    }
}

fn camera(name: &str, props: &Props, unit_scale: f32) -> Camera {
    let near = props.real("NearPlane", 10.0);
    let far = props.real("FarPlane", 4000.0);
    let width = props.real("AspectWidth", 640.0);
    let height = props.real("AspectHeight", 480.0);
    let aspect = match height > 0.0 {
        true => width / height,
        false => 1.0,
    };
    let projection = match props.integer("CameraProjectionType") {
        Some(1) => {
            let zoom = props.real("OrthoZoom", 1.0) / unit_scale.max(f32::MIN_POSITIVE);
            Projection::Orthographic {
                width: zoom * aspect,
                height: zoom,
                near,
                far,
            }
        }
        _ => Projection::Perspective {
            y_fov: props
                .number("FieldOfViewY")
                .or_else(|| props.number("FieldOfView"))
                .unwrap_or(45.0) as f32
                * std::f32::consts::PI
                / 180.0,
            aspect: Some(aspect),
            near,
            far: Some(far),
        },
    };
    Camera::new(name).projection(projection)
}

fn light(name: &str, props: &Props) -> Light {
    let range = match props.flag("EnableFarAttenuation").unwrap_or(false) {
        true => props.real("FarAttenuationEnd", 0.0),
        false => f32::INFINITY,
    };
    let kind = match props.integer("LightType") {
        Some(1) => LightKind::Directional,
        Some(2) => LightKind::Spot {
            range,
            inner_angle: props.real("InnerAngle", 0.0).to_radians() * 0.5,
            outer_angle: props.real("OuterAngle", 45.0).to_radians() * 0.5,
        },
        _ => LightKind::Point { range },
    };
    Light::new(name, kind)
        .color(props.vec3("Color", Vec3::ONE))
        .intensity(props.real("Intensity", 100.0) * 0.01)
}

fn parents(graph: &Graph<'_>, models: &[&Object<'_>]) -> FxHashMap<i64, i64> {
    models
        .iter()
        .filter_map(|object| {
            let parent = graph
                .destinations(object.id)
                .find(|(destination, property)| {
                    destination.kind == "Model" && property.is_none()
                })?;
            Some((object.id, parent.0.id))
        })
        .collect()
}

fn ordered<'a>(models: &[&'a Object<'a>], parents: &FxHashMap<i64, i64>) -> Vec<&'a Object<'a>> {
    let mut children: FxHashMap<i64, Vec<usize>> = FxHashMap::default();
    let mut roots = Vec::new();
    for (index, object) in models.iter().enumerate() {
        match parents.get(&object.id) {
            Some(parent) => children.entry(*parent).or_default().push(index),
            None => roots.push(index),
        }
    }

    let mut ordered = Vec::with_capacity(models.len());
    let mut visited = vec![false; models.len()];
    let mut stack: Vec<usize> = roots.into_iter().rev().collect();
    while let Some(index) = stack.pop() {
        if std::mem::replace(&mut visited[index], true) {
            continue;
        }
        ordered.push(models[index]);
        if let Some(child_indices) = children.get(&models[index].id) {
            stack.extend(child_indices.iter().rev().copied());
        }
    }
    for (index, object) in models.iter().enumerate() {
        if !visited[index] {
            ordered.push(object);
        }
    }
    ordered
}
