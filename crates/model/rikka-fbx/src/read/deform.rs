use rikka_model_core::glam::Mat4;
use rustc_hash::FxHashMap;

use crate::document::Node;
use crate::read::geometry::{Shape, triples};
use crate::read::graph::{Graph, Object};
use rikka_model_core::ImportCtx;
use rikka_model_core::ModelBuilder;
use rikka_model_core::NodeId;
use rikka_model_core::Result;
use rikka_model_core::Warning;
use rikka_model_core::{MorphTarget, Repeats};
use rikka_model_core::{Skin, SkinId};

pub(crate) struct Bound<'a> {
    pub nodes: &'a FxHashMap<i64, NodeId>,
    pub world: &'a FxHashMap<i64, Mat4>,
    pub mesh: Mat4,
}

pub(crate) fn skin(
    geometry: &Object<'_>,
    shape: &mut Shape,
    graph: &Graph<'_>,
    bound: &Bound<'_>,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<SkinId>> {
    let Some(deformer) = graph
        .sources(geometry.id)
        .map(|(object, _)| object)
        .find(|object| object.is("Deformer", "Skin"))
    else {
        return Ok(None);
    };

    let mut joints: Vec<NodeId> = Vec::new();
    let mut inverse_bind: Vec<Mat4> = Vec::new();
    let mut influences: Vec<Vec<(u16, f32)>> = vec![Vec::new(); shape.vertices_of_control.len()];

    for (cluster, _) in graph.sources(deformer.id) {
        ctx.checkpoint()?;
        if !cluster.is("Deformer", "Cluster") {
            continue;
        }
        let Some((bone, &joint)) = graph
            .sources(cluster.id)
            .filter(|(object, _)| object.kind == "Model")
            .find_map(|(object, _)| Some((object, bound.nodes.get(&object.id)?)))
        else {
            continue;
        };

        let slot = joints.len() as u16;
        joints.push(joint);
        inverse_bind.push(bind(
            cluster,
            bound.world.get(&bone.id).copied(),
            bound.mesh,
        ));

        let indices = cluster
            .node
            .child("Indexes")
            .map(Node::ints)
            .unwrap_or_default();
        let weights = cluster
            .node
            .child("Weights")
            .map(Node::doubles)
            .unwrap_or_default();
        for (&control, &weight) in indices.iter().zip(weights.iter()) {
            let Ok(control) = usize::try_from(control) else {
                continue;
            };
            if let Some(control_influences) = influences.get_mut(control)
                && weight > 0.0
            {
                control_influences.push((slot, weight as f32));
            }
        }
    }

    if joints.is_empty() {
        return Ok(None);
    }
    bind_vertices(shape, &influences);
    Ok(Some(builder.add_skin(Skin {
        name: deformer.name.clone(),
        joints,
        inverse_bind,
        skeleton: None,
    })))
}

fn bind(cluster: &Object<'_>, bone_world: Option<Mat4>, mesh_world: Mat4) -> Mat4 {
    if let Some(matrix) = matrix(cluster.node, "Transform") {
        return matrix;
    }
    let link = matrix(cluster.node, "TransformLink").or(bone_world);
    match link {
        Some(link) => link.inverse() * mesh_world,
        None => Mat4::IDENTITY,
    }
}

fn matrix(node: &Node, name: &str) -> Option<Mat4> {
    let doubles = node.child(name)?.doubles();
    let values: [f64; 16] = doubles.get(..16)?.try_into().ok()?;
    let matrix = Mat4::from_cols_array(&values.map(|value| value as f32));
    (matrix != Mat4::ZERO).then_some(matrix)
}

fn bind_vertices(shape: &mut Shape, influences: &[Vec<(u16, f32)>]) {
    for (control, control_influences) in influences.iter().enumerate() {
        for &vertex in &shape.vertices_of_control[control] {
            shape
                .mesh
                .vertices
                .set_influences(vertex as usize, control_influences);
        }
    }
}

pub(crate) fn morphs(
    geometry: &Object<'_>,
    shape: &mut Shape,
    graph: &Graph<'_>,
    ctx: &mut ImportCtx<'_>,
) -> Result<Vec<(i64, f32)>> {
    let mut weights = Vec::new();
    for (deformer, _) in graph.sources(geometry.id) {
        if !deformer.is("Deformer", "BlendShape") {
            continue;
        }
        for (channel, _) in graph.sources(deformer.id) {
            ctx.checkpoint()?;
            if !channel.is("Deformer", "BlendShapeChannel") {
                continue;
            }
            let shapes: Vec<&Object<'_>> = graph
                .sources(channel.id)
                .map(|(object, _)| object)
                .filter(|object| object.kind == "Geometry" && object.sub_kind == "Shape")
                .collect();
            if shapes.len() > 1 {
                ctx.warn(Warning::Unsupported(format!(
                    "blend channel \"{}\" has {} in-between shapes, reading only the last",
                    channel.name,
                    shapes.len()
                )));
            }
            let Some(last) = shapes.last() else {
                continue;
            };
            let Some(target) = target(last, &channel.name, shape) else {
                continue;
            };
            shape.mesh.morph_targets.push(target);
            let percent = channel
                .props
                .number("DeformPercent")
                .or_else(|| channel.node.child_number("DeformPercent"))
                .unwrap_or(0.0);
            weights.push((channel.id, percent as f32 / 100.0));
        }
    }
    Ok(weights)
}

fn target(shape_object: &Object<'_>, name: &str, shape: &Shape) -> Option<MorphTarget> {
    let node = shape_object.node;
    let indices = node.child("Indexes")?.ints();
    let positions = triples(node.child("Vertices")?);
    let normals = node.child("Normals").map(triples).unwrap_or_default();

    let mut target = MorphTarget::new(name);
    for (slot, &control) in indices.iter().enumerate() {
        let Ok(control) = usize::try_from(control) else {
            continue;
        };
        let Some(vertices) = shape.vertices_of_control.get(control) else {
            continue;
        };
        let Some(&position) = positions.get(slot) else {
            continue;
        };
        for &vertex in vertices {
            target.push(vertex, position, normals.get(slot).copied());
        }
    }
    target.merge(Repeats::KeepFirst);
    (!target.vertices.is_empty()).then_some(target)
}
