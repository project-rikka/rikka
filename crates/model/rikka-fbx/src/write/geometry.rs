use std::hash::Hash;

use rikka_model_core::glam::{Vec2, Vec3, Vec4};
use rustc_hash::FxHashMap;

use crate::document::{Node, Value};
use crate::write::topology::{ControlPoints, Topology};
use crate::write::{Composer, doubles, named};
use rikka_model_core::ExportCtx;
use rikka_model_core::MaterialId;
use rikka_model_core::Model;
use rikka_model_core::Result;
use rikka_model_core::{Mesh, MeshId};

const GEOMETRY_VERSION: i32 = 124;
const NORMAL_VERSION: i32 = 101;
const BINORMAL_VERSION: i32 = 101;
const TANGENT_VERSION: i32 = 101;
const SMOOTHING_VERSION: i32 = 102;
const COLOR_VERSION: i32 = 101;
const UV_VERSION: i32 = 101;
const MATERIAL_VERSION: i32 = 101;
const LAYER_VERSION: i32 = 100;
const UV_SET: &str = "UVMap";
const COLOR_SET: &str = "Color";
const NO_MATERIAL: i32 = -1;

pub(crate) struct GeometryRecord {
    pub geometry: i64,
    pub materials: Vec<MaterialId>,
    pub points: ControlPoints,
}

#[derive(Default)]
pub(crate) struct Meshes {
    records: Vec<Option<GeometryRecord>>,
}

impl Meshes {
    pub(crate) fn get(&self, id: MeshId) -> Option<&GeometryRecord> {
        self.records.get(id.index())?.as_ref()
    }
}

pub(crate) fn write(
    model: &Model,
    composer: &mut Composer,
    ctx: &mut ExportCtx<'_>,
) -> Result<Meshes> {
    let mut meshes = Meshes::default();
    for mesh in &model.meshes {
        ctx.checkpoint()?;
        meshes.records.push(match mesh.vertices.is_empty() {
            true => None,
            false => {
                let id = composer.id();
                let topology = Topology::of(mesh);
                let materials: Vec<MaterialId> = mesh
                    .submeshes
                    .iter()
                    .filter_map(|submesh| submesh.material)
                    .collect();
                composer.add("Geometry", geometry(mesh, &topology, id));
                Some(GeometryRecord {
                    geometry: id,
                    materials,
                    points: topology.points,
                })
            }
        });
    }
    Ok(meshes)
}

fn geometry(mesh: &Mesh, topology: &Topology<'_>, id: i64) -> Node {
    let vertices = &mesh.vertices;
    let edges = topology.edges(&vertices.normals);
    let mut node = Node::new("Geometry")
        .with_i64(id)
        .with_text(named(&mesh.name, "Geometry"))
        .with_text("Mesh")
        .adding(Node::new("GeometryVersion").with_i32(GEOMETRY_VERSION))
        .adding(
            Node::new("Vertices").with(doubles((0..topology.points.len()).flat_map(|point| {
                vertices.positions[topology.points.vertex(point) as usize].to_array()
            }))),
        )
        .adding(Node::new("PolygonVertexIndex").with(Value::I32s(topology.polygon_vertex_index())))
        .adding(Node::new("Edges").with(Value::I32s(edges.first_corners.clone())));

    let corner_vertices = || topology.corners.iter().map(|&vertex| vertex as usize);
    let mut layers: Vec<&str> = Vec::new();

    if !vertices.normals.is_empty() {
        let (normals, indices) = match mesh.morph_targets.is_empty() {
            true => indexed(corner_vertices().map(|vertex| {
                let normal = vertices.normals[vertex];
                (bits(normal.to_array()), normal)
            })),
            false => direct(corner_vertices().map(|vertex| vertices.normals[vertex])),
        };
        node.add(
            layer(
                "LayerElementNormal",
                NORMAL_VERSION,
                "ByPolygonVertex",
                "IndexToDirect",
                "",
            )
            .adding(Node::new("Normals").with(doubles(normals.iter().flat_map(Vec3::to_array))))
            .adding(Node::new("NormalsIndex").with(Value::I32s(indices))),
        );
        layers.push("LayerElementNormal");
    }

    if !vertices.tangents.is_empty() && !vertices.normals.is_empty() {
        let tangents = || corner_vertices().map(|vertex| vertices.tangents[vertex]);
        node.add(
            layer(
                "LayerElementBinormal",
                BINORMAL_VERSION,
                "ByPolygonVertex",
                "Direct",
                UV_SET,
            )
            .adding(
                Node::new("Binormals").with(doubles(tangents().zip(corner_vertices()).flat_map(
                    |(tangent, vertex)| {
                        (vertices.normals[vertex].cross(tangent.truncate()) * tangent.w).to_array()
                    },
                ))),
            ),
        );
        node.add(
            layer(
                "LayerElementTangent",
                TANGENT_VERSION,
                "ByPolygonVertex",
                "Direct",
                UV_SET,
            )
            .adding(Node::new("Tangents").with(doubles(
                tangents().flat_map(|tangent| tangent.truncate().to_array()),
            ))),
        );
        layers.push("LayerElementBinormal");
        layers.push("LayerElementTangent");
    }

    if !vertices.normals.is_empty() {
        node.add(
            layer(
                "LayerElementSmoothing",
                SMOOTHING_VERSION,
                "ByEdge",
                "Direct",
                "",
            )
            .adding(Node::new("Smoothing").with(Value::I32s(edges.smoothing()))),
        );
        layers.push("LayerElementSmoothing");
    }

    if !vertices.colors.is_empty() {
        let (colors, indices) = indexed(corner_vertices().map(|vertex| {
            let color = vertices.colors[vertex];
            (color, Vec4::from_array(color.map(f32::from)) / 255.0)
        }));
        node.add(
            layer(
                "LayerElementColor",
                COLOR_VERSION,
                "ByPolygonVertex",
                "IndexToDirect",
                COLOR_SET,
            )
            .adding(Node::new("Colors").with(doubles(colors.iter().flat_map(Vec4::to_array))))
            .adding(Node::new("ColorIndex").with(Value::I32s(indices))),
        );
        layers.push("LayerElementColor");
    }

    if !vertices.uvs.is_empty() {
        let (uvs, indices) = indexed(topology.corners.iter().enumerate().map(
            |(corner, &vertex)| {
                let uv = vertices.uvs[vertex as usize];
                let flipped = Vec2::new(uv.x, 1.0 - uv.y);
                ((bits(flipped.to_array()), topology.point(corner)), flipped)
            },
        ));
        node.add(
            layer(
                "LayerElementUV",
                UV_VERSION,
                "ByPolygonVertex",
                "IndexToDirect",
                UV_SET,
            )
            .adding(Node::new("UV").with(doubles(uvs.iter().flat_map(Vec2::to_array))))
            .adding(Node::new("UVIndex").with(Value::I32s(indices))),
        );
        layers.push("LayerElementUV");
    }

    let slots = material_slots(mesh);
    if slots.iter().any(|&slot| slot != NO_MATERIAL) {
        let materials: Vec<i32> = topology
            .submeshes
            .iter()
            .map(|&submesh| slots[submesh as usize])
            .collect();
        node.add(
            layer(
                "LayerElementMaterial",
                MATERIAL_VERSION,
                "ByPolygon",
                "IndexToDirect",
                "",
            )
            .adding(Node::new("Materials").with(Value::I32s(materials))),
        );
        layers.push("LayerElementMaterial");
    }

    let mut layer_node = Node::new("Layer")
        .with_i32(0)
        .adding(Node::new("Version").with_i32(LAYER_VERSION));
    for name in layers {
        layer_node.add(
            Node::new("LayerElement")
                .adding(Node::new("Type").with_text(name))
                .adding(Node::new("TypedIndex").with_i32(0)),
        );
    }
    node.adding(layer_node)
}

fn material_slots(mesh: &Mesh) -> Vec<i32> {
    let mut next = 0;
    mesh.submeshes
        .iter()
        .map(|submesh| match submesh.material {
            Some(_) => {
                next += 1;
                next - 1
            }
            None => NO_MATERIAL,
        })
        .collect()
}

fn indexed<K: Eq + Hash, V>(values: impl Iterator<Item = (K, V)>) -> (Vec<V>, Vec<i32>) {
    let mut unique = Vec::new();
    let mut found: FxHashMap<K, i32> = FxHashMap::default();
    let indices = values
        .map(|(key, value)| {
            *found.entry(key).or_insert_with(|| {
                unique.push(value);
                unique.len() as i32 - 1
            })
        })
        .collect();
    (unique, indices)
}

fn direct<V>(values: impl Iterator<Item = V>) -> (Vec<V>, Vec<i32>) {
    let values: Vec<V> = values.collect();
    let indices = (0..values.len() as i32).collect();
    (values, indices)
}

fn bits<const N: usize>(values: [f32; N]) -> [u32; N] {
    values.map(|value| (value + 0.0).to_bits())
}

fn layer(name: &str, version: i32, mapping: &str, reference: &str, set: &str) -> Node {
    Node::new(name)
        .with_i32(0)
        .adding(Node::new("Version").with_i32(version))
        .adding(Node::new("Name").with_text(set))
        .adding(Node::new("MappingInformationType").with_text(mapping))
        .adding(Node::new("ReferenceInformationType").with_text(reference))
}
