mod anim;
pub(crate) mod ascii;
pub(crate) mod binary;
mod curve;
mod deform;
mod geometry;
mod graph;
mod material;
pub(crate) mod props;
mod scene;
mod settings;
mod transform;

use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;

use crate::document::Node as Record;
use crate::{KTIME, KTIME_LATER};
use rikka_model_core::MaterialId;
use rikka_model_core::MeshId;
use rikka_model_core::Model;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Result;
use rikka_model_core::SkinId;
use rikka_model_core::Stage;
use rikka_model_core::{ImportCtx, Input};

use anim::BlendShapeSlot;
use geometry::Shape;
use graph::{Graph, Object};
use settings::Settings;

const LATER_TIME_VERSION: u32 = 8000;
const ORIGINAL_TIME_DEFINITION: i64 = 127;

pub(crate) fn read(input: &Input<'_>, ctx: &mut ImportCtx<'_>) -> Result<Model> {
    let text = !input.bytes().starts_with(binary::MAGIC);
    let (version, records) = match text {
        true => ascii::parse(input.bytes(), ctx)?,
        false => binary::parse(input.bytes(), ctx)?,
    };
    ctx.report(Stage::Parsing, 0.3);

    let settings = Settings::read(&records);
    let graph = Graph::read(&records, version, text);
    let mut builder = ModelBuilder::new()
        .coordinates(settings.coordinates)
        .unit_scale(settings.unit_scale);
    if let Some(creator) = creator(&records) {
        builder = builder.generator(creator);
    }

    let material_table = material::read(&graph, &mut builder, ctx)?;
    ctx.report(Stage::Parsing, 0.5);
    let scene = scene::read(&graph, &mut builder, ctx)?;
    ctx.report(Stage::Parsing, 0.6);

    let blend_shapes = draw(&graph, &scene, &material_table, &mut builder, ctx)?;
    ctx.report(Stage::Parsing, 0.85);

    anim::read(
        &records,
        &graph,
        &scene,
        &blend_shapes,
        ktime(&records, version),
        &mut builder,
        ctx,
    )?;
    ctx.report(Stage::Parsing, 1.0);
    builder.finish()
}

fn draw(
    graph: &Graph<'_>,
    scene: &scene::Scene,
    material_table: &material::MaterialTable,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<FxHashMap<i64, BlendShapeSlot>> {
    let mut shapes: FxHashMap<i64, Option<MeshGeometry>> = FxHashMap::default();
    let mut meshes: FxHashMap<(i64, Vec<Option<MaterialId>>), MeshId> = FxHashMap::default();
    let mut blend_shapes: FxHashMap<i64, BlendShapeSlot> = FxHashMap::default();

    for (index, &model) in scene.models.iter().enumerate() {
        ctx.checkpoint()?;
        ctx.report(
            Stage::Parsing,
            0.6 + 0.25 * index as f32 / scene.models.len().max(1) as f32,
        );
        let Some(object) = graph.object(model) else {
            continue;
        };
        let Some(geometry) = graph
            .sources(model)
            .map(|(source, _)| source)
            .find(|source| source.kind == "Geometry" && source.sub_kind != "Shape")
            .or_else(|| inline(object))
        else {
            continue;
        };

        let slots: Vec<Option<MaterialId>> = graph
            .sources(model)
            .filter(|(source, _)| source.kind == "Material")
            .map(|(source, _)| material_table.materials.get(&source.id).copied())
            .collect();

        let mesh_geometry = match shapes.entry(geometry.id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(slot) => {
                slot.insert(build(geometry, object, graph, scene, builder, ctx)?)
            }
        };
        let Some(mesh_geometry) = mesh_geometry.as_ref() else {
            continue;
        };

        let carrier = scene.carriers[&model];
        let mesh = match meshes.get(&(geometry.id, slots.clone())) {
            Some(&mesh_id) => mesh_id,
            None => {
                let mut mesh = mesh_geometry.shape.mesh.clone();
                for (submesh, slot) in mesh.submeshes.iter_mut().zip(&mesh_geometry.shape.slots) {
                    submesh.material = slots.get(*slot as usize).copied().flatten();
                }
                let mesh_id = builder.add_mesh(mesh);
                meshes.insert((geometry.id, slots), mesh_id);
                mesh_id
            }
        };

        let node = &mut builder.model_mut().nodes[carrier.index()];
        node.mesh = Some(mesh);
        node.skin = mesh_geometry.skin;
        node.morph_weights = mesh_geometry
            .weights
            .iter()
            .map(|&(_, weight)| weight)
            .collect();

        for (slot, &(channel, _)) in mesh_geometry.weights.iter().enumerate() {
            blend_shapes.insert(
                channel,
                BlendShapeSlot {
                    node: carrier,
                    slot,
                    count: mesh_geometry.weights.len(),
                },
            );
        }
    }
    Ok(blend_shapes)
}

fn inline<'a>(object: &'a Object<'a>) -> Option<&'a Object<'a>> {
    let has_geometry = object.node.child("Vertices").is_some()
        && object.node.child("PolygonVertexIndex").is_some();
    has_geometry.then_some(object)
}
struct MeshGeometry {
    shape: Shape,
    skin: Option<SkinId>,
    weights: Vec<(i64, f32)>,
}

fn build(
    geometry: &Object<'_>,
    model: &Object<'_>,
    graph: &Graph<'_>,
    scene: &scene::Scene,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<MeshGeometry>> {
    let Some(mut shape) = geometry::read(geometry, ctx)? else {
        return Ok(None);
    };
    let skin = deform::skin(
        geometry,
        &mut shape,
        graph,
        &deform::Bound {
            nodes: &scene.nodes,
            world: &scene.world,
            mesh: scene.world.get(&model.id).copied().unwrap_or_default(),
        },
        builder,
        ctx,
    )?;
    let weights = deform::morphs(geometry, &mut shape, graph, ctx)?;
    Ok(Some(MeshGeometry {
        shape,
        skin,
        weights,
    }))
}

fn creator(records: &[Record]) -> Option<String> {
    let first_text = |node: &Record| node.text(0).map(str::to_owned);
    records
        .iter()
        .find(|node| node.name == "Creator")
        .and_then(first_text)
        .or_else(|| {
            records
                .iter()
                .find(|node| node.name == "FBXHeaderExtension")?
                .child("Creator")
                .and_then(first_text)
        })
}

fn ktime(records: &[Record], version: u32) -> i64 {
    let header = records
        .iter()
        .find(|node| node.name == "FBXHeaderExtension");
    let declared = header
        .filter(|node| node.child_integer("FBXHeaderVersion").unwrap_or(0) >= 1004)
        .and_then(|node| node.child_integer("TCDefinition"));
    let original = match declared {
        Some(definition) => definition == ORIGINAL_TIME_DEFINITION,
        None => version < LATER_TIME_VERSION,
    };
    match original {
        true => KTIME,
        false => KTIME_LATER,
    }
}
