use rikka_model_core::glam::{Mat4, Vec3};
use rustc_hash::FxHashMap;

use crate::document::{Node, Value};
use crate::write::geometry::{GeometryRecord, Meshes};
use crate::write::scene::NodeIds;
use crate::write::{Composer, doubles, named, prop};
use rikka_model_core::ExportCtx;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::SkinId;
use rikka_model_core::{INFLUENCES, MeshId, MorphTarget};

#[derive(Default)]
pub(crate) struct BlendChannels {
    channels: FxHashMap<(MeshId, usize), i64>,
}

impl BlendChannels {
    pub(crate) fn channel(&self, mesh: MeshId, slot: usize) -> Option<i64> {
        self.channels.get(&(mesh, slot)).copied()
    }
}

pub(crate) fn write(
    model: &Model,
    composer: &mut Composer,
    meshes: &Meshes,
    node_ids: &NodeIds,
    ctx: &mut ExportCtx<'_>,
) -> Result<BlendChannels> {
    let mut shaped = BlendChannels::default();
    let mut written: Vec<(MeshId, SkinId)> = Vec::new();

    for node in &model.nodes {
        let (Some(mesh), Some(skin)) = (node.mesh, node.skin) else {
            continue;
        };
        if written.contains(&(mesh, skin)) {
            continue;
        }
        written.push((mesh, skin));
        let Some(record) = meshes.get(mesh) else {
            continue;
        };
        ctx.checkpoint()?;
        bind(model, composer, node_ids, record, mesh, skin);
    }

    for (index, mesh) in model.meshes.iter().enumerate() {
        if mesh.morph_targets.is_empty() {
            continue;
        }
        let Some(record) = meshes.get(MeshId(index as u32)) else {
            continue;
        };
        ctx.checkpoint()?;
        blend(model, composer, &mut shaped, record, MeshId(index as u32));
    }
    Ok(shaped)
}

fn bind(
    model: &Model,
    composer: &mut Composer,
    node_ids: &NodeIds,
    record: &GeometryRecord,
    mesh: MeshId,
    skin_id: SkinId,
) {
    let skin = model.skin(skin_id);
    let vertices = &model.mesh(mesh).vertices;
    let id = composer.id();
    composer.add(
        "Deformer",
        Node::new("Deformer")
            .with_i64(id)
            .with_text(named(&skin.name, "Deformer"))
            .with_text("Skin")
            .adding(Node::new("Version").with_i32(101))
            .adding(Node::new("Link_DeformAcceptance").with_f64(0.0))
            .adding(Node::new("SkinningType").with_text("Linear")),
    );
    composer.connect(id, record.geometry);

    let mut influences: Vec<Vec<(i32, f64)>> = vec![Vec::new(); skin.joints.len()];
    if !vertices.joints.is_empty() {
        for point in 0..record.points.len() {
            let vertex = record.points.vertex(point) as usize;
            let (joints, weights) = (vertices.joints[vertex], vertices.weights[vertex]);
            for slot in 0..INFLUENCES {
                let weight = weights[slot];
                if weight > 0.0
                    && let Some(pulls) = influences.get_mut(usize::from(joints[slot]))
                {
                    pulls.push((point as i32, f64::from(weight)));
                }
            }
        }
    }

    for (slot, joint) in skin.joints.iter().enumerate() {
        let cluster = composer.id();
        let bind = skin.inverse_bind[slot];
        let mut node = Node::new("Deformer")
            .with_i64(cluster)
            .with_text(named(&model.node(*joint).name, "SubDeformer"))
            .with_text("Cluster")
            .adding(Node::new("Version").with_i32(100))
            .adding(Node::new("UserData").with_text("").with_text(""));
        if !influences[slot].is_empty() {
            node.add(
                Node::new("Indexes").with(Value::I32s(
                    influences[slot]
                        .iter()
                        .map(|influence| influence.0)
                        .collect(),
                )),
            );
            node.add(
                Node::new("Weights").with(Value::F64s(
                    influences[slot]
                        .iter()
                        .map(|influence| influence.1)
                        .collect(),
                )),
            );
        }
        node.add(Node::new("Transform").with(doubles(bind.to_cols_array())));
        node.add(Node::new("TransformLink").with(doubles(bind.inverse().to_cols_array())));
        node.add(
            Node::new("TransformAssociateModel").with(doubles(Mat4::IDENTITY.to_cols_array())),
        );

        composer.add("Deformer", node);
        composer.connect(cluster, id);
        composer.connect(node_ids.node(*joint), cluster);
    }
}

fn blend(
    model: &Model,
    composer: &mut Composer,
    shaped: &mut BlendChannels,
    record: &GeometryRecord,
    mesh_id: MeshId,
) {
    let mesh = model.mesh(mesh_id);
    let id = composer.id();
    composer.add(
        "Deformer",
        Node::new("Deformer")
            .with_i64(id)
            .with_text(named(&mesh.name, "Deformer"))
            .with_text("BlendShape")
            .adding(Node::new("Version").with_i32(100)),
    );
    composer.connect(id, record.geometry);

    let weights = model
        .nodes
        .iter()
        .find(|node| node.mesh == Some(mesh_id))
        .map_or(&[][..], |node| node.morph_weights.as_slice());

    for (slot, target) in mesh.morph_targets.iter().enumerate() {
        let channel = composer.id();
        let weight = weights.get(slot).copied().unwrap_or(0.0) * 100.0;
        composer.add(
            "Deformer",
            Node::new("Deformer")
                .with_i64(channel)
                .with_text(named(&target.name, "SubDeformer"))
                .with_text("BlendShapeChannel")
                .adding(Node::new("Version").with_i32(100))
                .adding(Node::new("DeformPercent").with_f64(f64::from(weight)))
                .adding(Node::new("FullWeights").with(Value::F64s(vec![100.0])))
                .adding(
                    Node::new("Properties70").adding(
                        prop("DeformPercent", "Number", "", "A").with_f64(f64::from(weight)),
                    ),
                ),
        );
        composer.connect(channel, id);

        let shape = composer.id();
        let offsets = point_offsets(target, record);
        let mut node = Node::new("Geometry")
            .with_i64(shape)
            .with_text(named(&target.name, "Geometry"))
            .with_text("Shape")
            .adding(Node::new("Version").with_i32(100))
            .adding(Node::new("Indexes").with(Value::I32s(
                offsets.iter().map(|offset| offset.point as i32).collect(),
            )))
            .adding(Node::new("Vertices").with(doubles(
                offsets.iter().flat_map(|offset| offset.position.to_array()),
            )));
        if !target.normals.is_empty() {
            node.add(Node::new("Normals").with(doubles(
                offsets.iter().flat_map(|offset| offset.normal.to_array()),
            )));
        }
        composer.add("Geometry", node);
        composer.connect(shape, channel);
        shaped.channels.insert((mesh_id, slot), channel);
    }
}

struct PointOffset {
    point: u32,
    position: Vec3,
    normal: Vec3,
}

fn point_offsets(target: &MorphTarget, record: &GeometryRecord) -> Vec<PointOffset> {
    let mut offsets: Vec<PointOffset> = target
        .vertices
        .iter()
        .zip(&target.positions)
        .enumerate()
        .map(|(slot, (&vertex, &position))| PointOffset {
            point: record.points.of_vertex(vertex),
            position,
            normal: target.normals.get(slot).copied().unwrap_or(Vec3::ZERO),
        })
        .collect();
    offsets.sort_by_key(|offset| offset.point);
    offsets.dedup_by_key(|offset| offset.point);
    offsets
}
