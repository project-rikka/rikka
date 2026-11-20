use std::borrow::Cow;

use rikka_model_core::glam::{Vec2, Vec3, Vec4};

use crate::document::Node;
use crate::read::graph::Object;
use rikka_model_core::ImportCtx;
use rikka_model_core::Mesh;
use rikka_model_core::Result;
use rikka_model_core::Warning;
use rikka_model_core::polygon;
use rikka_model_core::{Corner as MeshCorner, MeshAssembler};

pub(crate) struct Shape {
    pub mesh: Mesh,
    pub slots: Vec<u32>,
    pub vertices_of_control: Vec<Vec<u32>>,
}

pub(crate) fn read(object: &Object<'_>, ctx: &mut ImportCtx<'_>) -> Result<Option<Shape>> {
    ctx.checkpoint()?;
    let node = object.node;
    let Some(points) = node.child("Vertices").map(triples) else {
        return Ok(None);
    };
    let Some(corners) = node.child("PolygonVertexIndex") else {
        return Ok(None);
    };
    if points.is_empty() {
        return Ok(None);
    }

    let layers = Layers::read(node);
    warn_about_extras(node, &object.name, ctx);

    let mut builder = Builder {
        points: &points,
        layers: &layers,
        assembler: MeshAssembler::new(),
        outline: Vec::new(),
        used: vec![false; points.len()],
    };
    let mut polygon = 0;
    let mut face: Vec<Corner> = Vec::new();
    for (index, corner) in corners.ints().iter().enumerate() {
        let last = *corner < 0;
        let control = match last {
            true => !*corner,
            false => *corner,
        };
        if control >= 0 && (control as usize) < points.len() {
            face.push(Corner {
                control: control as usize,
                at: index,
            });
        }
        if !last {
            continue;
        }
        if polygon % CHECKED_POLYGONS == 0 {
            ctx.checkpoint()?;
        }
        let slot = layers.materials.at(index, 0, polygon).unwrap_or(0);
        builder.polygon(&face, polygon, slot);
        face.clear();
        polygon += 1;
    }
    if let Some(last) = face.last() {
        let slot = layers.materials.at(last.at, 0, polygon).unwrap_or(0);
        builder.polygon(&face, polygon, slot);
    }
    builder.loose_points();
    ctx.checkpoint()?;

    let assembled = builder.assembler.finish(&object.name);
    Ok(Some(Shape {
        vertices_of_control: assembled.vertices_of_controls(points.len()),
        mesh: assembled.mesh,
        slots: assembled.slots,
    }))
}

const CHECKED_POLYGONS: usize = 4096;
const UNASSIGNED_SLOT: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Corner {
    control: usize,
    at: usize,
}

struct Builder<'a> {
    points: &'a [Vec3],
    layers: &'a Layers<'a>,
    assembler: MeshAssembler,
    outline: Vec<u32>,
    used: Vec<bool>,
}

impl Builder<'_> {
    fn polygon(&mut self, face: &[Corner], polygon: usize, slot: u32) {
        if face.len() < 3 {
            return;
        }
        let layers = self.layers;
        let flat = polygon::normal(face.iter().map(|corner| self.points[corner.control]))
            .unwrap_or(Vec3::Y);

        self.outline.clear();
        for corner in face {
            let (at, control) = (corner.at, corner.control);
            self.used[control] = true;
            let normal = layers.normals.at(at, control, polygon).unwrap_or(flat);
            let tangent = layers.tangents.at(at, control, polygon);
            let binormal = layers.binormals.at(at, control, polygon);
            let vertex = self.assembler.corner(
                control as u32,
                MeshCorner {
                    position: self.points[control],
                    normal,
                    tangent: handed(tangent, binormal, normal),
                    uv: layers
                        .uvs
                        .at(at, control, polygon)
                        .map(|uv| Vec2::new(uv.x, 1.0 - uv.y)),
                    color: layers
                        .colors
                        .at(at, control, polygon)
                        .map(MeshCorner::color_from),
                },
            );
            self.outline.push(vertex);
        }
        self.assembler.polygon(&self.outline, slot);
    }

    fn loose_points(&mut self) {
        let layers = self.layers;
        if !self.used.contains(&true) {
            return;
        }
        for control in 0..self.points.len() {
            if self.used[control] {
                continue;
            }
            let normal = layers.normals.at_point(control).unwrap_or(Vec3::Y);
            self.assembler.corner(
                control as u32,
                MeshCorner {
                    position: self.points[control],
                    normal,
                    tangent: handed(
                        layers.tangents.at_point(control),
                        layers.binormals.at_point(control),
                        normal,
                    ),
                    uv: layers
                        .uvs
                        .at_point(control)
                        .map(|uv| Vec2::new(uv.x, 1.0 - uv.y)),
                    color: layers.colors.at_point(control).map(MeshCorner::color_from),
                },
            );
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mapping {
    Corner,
    Point,
    Polygon,
    Whole,
    Unread,
}

struct Layers<'a> {
    normals: Layer<'a, Vec3>,
    tangents: Layer<'a, Vec3>,
    binormals: Layer<'a, Vec3>,
    uvs: Layer<'a, Vec2>,
    colors: Layer<'a, Vec4>,
    materials: Layer<'a, u32>,
}

impl<'a> Layers<'a> {
    fn read(node: &'a Node) -> Self {
        Self {
            normals: Layer::read(
                node,
                "LayerElementNormal",
                "Normals",
                "NormalsIndex",
                triples,
            ),
            tangents: Layer::read(
                node,
                "LayerElementTangent",
                "Tangents",
                "TangentsIndex",
                triples,
            ),
            binormals: Layer::read(
                node,
                "LayerElementBinormal",
                "Binormals",
                "BinormalsIndex",
                triples,
            ),
            uvs: Layer::read(node, "LayerElementUV", "UV", "UVIndex", pairs),
            colors: Layer::read(node, "LayerElementColor", "Colors", "ColorIndex", quads),
            materials: Layer::read(node, "LayerElementMaterial", "Materials", "", |node| {
                node.ints()
                    .iter()
                    .map(|&slot| u32::try_from(slot).unwrap_or(UNASSIGNED_SLOT))
                    .collect()
            }),
        }
    }
}

struct Layer<'a, T> {
    values: Vec<T>,
    indices: Cow<'a, [i32]>,
    mapping: Mapping,
}

impl<'a, T: Copy> Layer<'a, T> {
    fn read(
        geometry: &'a Node,
        block: &str,
        values: &str,
        indices: &str,
        read: impl Fn(&Node) -> Vec<T>,
    ) -> Self {
        let Some(layer) = geometry.child(block) else {
            return Self::none();
        };
        let mapping = match layer.child_text("MappingInformationType") {
            Some("ByPolygonVertex") => Mapping::Corner,
            Some("ByVertex" | "ByVertice" | "ByVertices" | "ByControlPoint") => Mapping::Point,
            Some("ByPolygon") => Mapping::Polygon,
            Some("AllSame") => Mapping::Whole,
            _ => Mapping::Unread,
        };
        let direct = !matches!(
            layer.child_text("ReferenceInformationType"),
            Some("IndexToDirect" | "Index")
        );
        Self {
            values: layer.child(values).map(read).unwrap_or_default(),
            indices: match direct {
                true => Cow::Borrowed(&[]),
                false => layer.child(indices).map(Node::ints).unwrap_or_default(),
            },
            mapping,
        }
    }

    fn none() -> Self {
        Self {
            values: Vec::new(),
            indices: Cow::Borrowed(&[]),
            mapping: Mapping::Unread,
        }
    }

    fn at(&self, corner: usize, point: usize, polygon: usize) -> Option<T> {
        match self.mapping {
            Mapping::Corner => self.value(corner),
            Mapping::Point => self.value(point),
            Mapping::Polygon => self.value(polygon),
            Mapping::Whole => self.value(0),
            Mapping::Unread => None,
        }
    }

    fn at_point(&self, point: usize) -> Option<T> {
        match self.mapping {
            Mapping::Point => self.value(point),
            Mapping::Whole => self.value(0),
            Mapping::Corner | Mapping::Polygon | Mapping::Unread => None,
        }
    }

    fn value(&self, index: usize) -> Option<T> {
        let index = match self.indices.is_empty() {
            true => index,
            false => usize::try_from(*self.indices.get(index)?).ok()?,
        };
        self.values.get(index).copied()
    }
}

pub(crate) fn triples(node: &Node) -> Vec<Vec3> {
    node.doubles()
        .as_chunks::<3>()
        .0
        .iter()
        .map(|chunk| Vec3::new(chunk[0] as f32, chunk[1] as f32, chunk[2] as f32))
        .collect()
}

fn pairs(node: &Node) -> Vec<Vec2> {
    node.doubles()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| Vec2::new(chunk[0] as f32, chunk[1] as f32))
        .collect()
}

fn quads(node: &Node) -> Vec<Vec4> {
    node.doubles()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| {
            Vec4::new(
                chunk[0] as f32,
                chunk[1] as f32,
                chunk[2] as f32,
                chunk[3] as f32,
            )
        })
        .collect()
}

fn handed(tangent: Option<Vec3>, binormal: Option<Vec3>, normal: Vec3) -> Option<Vec4> {
    let tangent = tangent?;
    let sign = match binormal {
        Some(binormal) if normal.cross(tangent).dot(binormal) < 0.0 => -1.0,
        _ => 1.0,
    };
    Some(tangent.normalize_or(Vec3::X).extend(sign))
}

fn warn_about_extras(node: &Node, name: &str, ctx: &mut ImportCtx<'_>) {
    let sets = node.children_named("LayerElementUV").count();
    if sets > 1 {
        ctx.warn(Warning::Unsupported(format!(
            "mesh \"{name}\" has {sets} UV sets, reading only the first"
        )));
    }
}
