mod ascii;
mod binary;

use rikka_model_core::glam::{Vec3, Vec4};
use rikka_model_core::{Conversion, Error, ExportCtx, Model, Requirement, Result, Stage};
use rikka_types::srgb;

use crate::facet::{Facet, Solid};
use crate::{STL_COORDINATES, STL_UNIT_SCALE};

pub(crate) fn write(model: &Model, ctx: &mut ExportCtx<'_>, text: bool) -> Result<Vec<u8>> {
    let solids = solids(model, ctx)?;
    ctx.report(Stage::Writing, 0.5);
    match text {
        true => Ok(ascii::write(&solids)),
        false => binary::write(&solids),
    }
}

fn solids(model: &Model, ctx: &mut ExportCtx<'_>) -> Result<Vec<Solid>> {
    let conversion = Conversion::new(
        model.metadata.coordinates,
        STL_COORDINATES,
        model.metadata.unit_scale / STL_UNIT_SCALE,
    );
    let world = model.world_transforms();
    let painted = painted(model);
    let mut solids = Vec::new();
    for (index, node) in model.nodes.iter().enumerate() {
        ctx.checkpoint()?;
        let Some(mesh) = node.mesh else {
            continue;
        };
        let mesh = model.mesh(mesh);
        let placement = conversion.matrix() * world[index];
        let mirrored = placement.determinant() < 0.0;
        let mut facets = Vec::with_capacity(mesh.triangle_count());
        for submesh in &mesh.submeshes {
            let tint = submesh
                .material
                .map_or(Vec4::ONE, |material| model.material(material).base_color);
            let first = submesh.first_index as usize;
            let indices = &mesh.indices[first..first + submesh.index_count as usize];
            for triangle in indices.as_chunks::<3>().0 {
                let [a, b, c] = match mirrored {
                    true => [triangle[0], triangle[2], triangle[1]],
                    false => *triangle,
                };
                let corners = [a, b, c].map(|vertex| {
                    placement.transform_point3(mesh.vertices.positions[vertex as usize])
                });
                if corners.iter().any(|corner| !corner.is_finite()) {
                    return Err(Error::Unmet {
                        format: "STL",
                        requirement: Requirement::FiniteVertices(mesh.name.clone()),
                    });
                }
                let color = painted.then(|| {
                    let shade = [a, b, c]
                        .iter()
                        .filter_map(|&vertex| mesh.vertices.colors.get(vertex as usize))
                        .map(|color| Vec4::from_array(color.map(f32::from)).truncate() / 255.0)
                        .sum::<Vec3>();
                    let shade = match mesh.vertices.colors.is_empty() {
                        true => Vec3::ONE,
                        false => shade / 3.0,
                    };
                    (tint.truncate() * shade).to_array().map(encoded)
                });
                facets.push(Facet {
                    normal: normal(corners),
                    corners,
                    color,
                });
            }
        }
        solids.push(Solid {
            name: node.name.clone(),
            facets,
        });
    }
    Ok(solids)
}

fn painted(model: &Model) -> bool {
    let node_meshes = || {
        model
            .nodes
            .iter()
            .filter_map(|node| node.mesh)
            .map(|mesh| model.mesh(mesh))
    };
    if node_meshes().any(|mesh| !mesh.vertices.colors.is_empty()) {
        return true;
    }
    let mut tints = node_meshes()
        .flat_map(|mesh| &mesh.submeshes)
        .map(|submesh| {
            submesh
                .material
                .map_or(Vec4::ONE, |material| model.material(material).base_color)
                .truncate()
        });
    let Some(first) = tints.next() else {
        return false;
    };
    tints.any(|tint| !tint.abs_diff_eq(first, 1.0 / 512.0))
}

fn normal([a, b, c]: [Vec3; 3]) -> Vec3 {
    (b - a).cross(c - a).normalize_or_zero()
}

fn encoded(linear: f32) -> u8 {
    (srgb::from_linear(linear.clamp(0.0, 1.0)) * 255.0).round() as u8
}
