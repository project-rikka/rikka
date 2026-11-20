use rikka_model_core::glam::Vec3;
use rikka_model_core::{Corner, Mesh, MeshAssembler, Model, ModelBuilder, Node, Result};
use rikka_types::srgb;

use crate::facet::{Document, Facet, Solid};
use crate::{STL_COORDINATES, STL_UNIT_SCALE};

const NEUTRAL: u8 = 204;

pub(crate) fn build(document: Document, name: &str, generator: Option<String>) -> Result<Model> {
    let mut builder = ModelBuilder::new()
        .coordinates(STL_COORDINATES)
        .unit_scale(STL_UNIT_SCALE);
    if let Some(generator) = generator {
        builder = builder.generator(generator);
    }
    if document.solids.iter().all(|solid| solid.facets.is_empty()) {
        return builder.finish();
    }

    let painted = document.object_color.is_some()
        || document
            .solids
            .iter()
            .flat_map(|solid| &solid.facets)
            .any(|facet| facet.color.is_some());
    let unpainted = document
        .object_color
        .map_or([NEUTRAL; 3], |[red, green, blue, _]| {
            linear_bytes([red, green, blue])
        });

    let named = document.solids.len() == 1;
    for (index, solid) in document.solids.into_iter().enumerate() {
        if solid.facets.is_empty() {
            continue;
        }
        let title = match (solid.name.is_empty(), named) {
            (false, _) => solid.name.clone(),
            (true, true) if !name.is_empty() => name.to_owned(),
            _ => format!("solid{index}"),
        };
        let mesh = builder.add_mesh(mesh(&title, &solid, painted.then_some(unpainted)));
        builder.add_node(Node::new(title).mesh(mesh));
    }
    builder.finish()
}

fn mesh(name: &str, solid: &Solid, painted: Option<[u8; 3]>) -> Mesh {
    let mut assembler = MeshAssembler::new();
    for facet in &solid.facets {
        let normal = facing(facet);
        let color = painted.map(|unpainted| facet.color.map_or(unpainted, linear_bytes));
        let vertices = facet.corners.map(|position| {
            assembler.corner(
                0,
                Corner {
                    color: color.map(|[red, green, blue]| [red, green, blue, 255]),
                    ..Corner::new(position, normal)
                },
            )
        });
        assembler.triangle(vertices, 0);
    }
    let mut mesh = assembler.finish(name).mesh;
    for submesh in &mut mesh.submeshes {
        submesh.name = name.to_owned();
    }
    mesh
}

fn facing(facet: &Facet) -> Vec3 {
    let [a, b, c] = facet.corners;
    (b - a)
        .cross(c - a)
        .try_normalize()
        .or_else(|| facet.normal.try_normalize())
        .unwrap_or(Vec3::Z)
}

fn linear(encoded: u8) -> f32 {
    srgb::to_linear(f32::from(encoded) / 255.0)
}

fn linear_bytes(color: [u8; 3]) -> [u8; 3] {
    color.map(|channel| (linear(channel) * 255.0).round() as u8)
}
