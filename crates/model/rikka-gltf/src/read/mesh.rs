use std::collections::BTreeMap;

use rikka_model_core::glam::{Vec2, Vec3, Vec4};

use crate::document::{self, Document, Index, Shape, TRIANGLE_FAN, TRIANGLE_STRIP, TRIANGLES};
use crate::read::accessor::Accessors;
use rikka_model_core::Bounds;
use rikka_model_core::ImportCtx;
use rikka_model_core::Limit;
use rikka_model_core::MaterialId;
use rikka_model_core::ModelBuilder;
use rikka_model_core::Stage;
use rikka_model_core::Warning;
use rikka_model_core::{Error, Result};
use rikka_model_core::{INFLUENCES, Mesh, MeshId, MorphTarget, Submesh, Vertices};

#[derive(Debug, Default)]
struct Deltas {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
}

#[derive(Debug, Default)]
struct Piece {
    vertices: Vertices,
    indices: Vec<u32>,
    targets: Vec<Deltas>,
    material: Option<MaterialId>,
    unshared: bool,
}

#[derive(Clone, Copy, Debug)]
struct Shared {
    primitive: usize,
    base: u32,
    count: usize,
}

impl Shared {
    fn streams<'a>(
        &self,
        mesh: &'a document::Mesh,
    ) -> (&'a BTreeMap<String, Index>, &'a [BTreeMap<String, Index>]) {
        let primitive = &mesh.primitives[self.primitive];
        (&primitive.attributes, &primitive.targets)
    }
}

#[derive(Debug, Default)]
struct Merged {
    vertices: Vertices,
    indices: Vec<u32>,
    submeshes: Vec<Submesh>,
    targets: Vec<MorphTarget>,
    shaped: Vec<bool>,
}

pub(crate) fn read(
    document: &Document,
    accessors: &Accessors<'_>,
    builder: &mut ModelBuilder,
    ctx: &mut ImportCtx<'_>,
) -> Result<Vec<Option<MeshId>>> {
    let mut meshes = Vec::with_capacity(document.meshes.len());
    let mut read = Geometry::default();
    for (index, described) in document.meshes.iter().enumerate() {
        ctx.checkpoint()?;
        ctx.report(
            Stage::Parsing,
            index as f32 / document.meshes.len().max(1) as f32,
        );
        let name = described
            .name
            .clone()
            .unwrap_or_else(|| format!("mesh{index}"));
        let merged = merge(
            accessors,
            described,
            &name,
            document.materials.len(),
            &mut read,
            ctx,
        )?;
        meshes.push(match merged {
            Some(mesh) => Some(builder.add_mesh(mesh)),
            None => {
                ctx.warn(Warning::Unsupported(format!(
                    "skipping mesh \"{name}\" without supported primitives"
                )));
                None
            }
        });
    }
    Ok(meshes)
}

#[derive(Debug, Default)]
struct Geometry {
    vertices: usize,
    indices: usize,
}

impl Geometry {
    fn grow(&mut self, vertices: usize, indices: usize, ctx: &mut ImportCtx<'_>) -> Result<()> {
        self.vertices = self.vertices.saturating_add(vertices);
        self.indices = self.indices.saturating_add(indices);
        let limits = &ctx.options().limits;
        limits.check(Limit::Vertices, self.vertices as u64)?;
        limits.check(Limit::Indices, self.indices as u64)
    }
}

fn merge(
    accessors: &Accessors<'_>,
    described: &document::Mesh,
    name: &str,
    materials: usize,
    read: &mut Geometry,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<Mesh>> {
    let names = described
        .extras
        .as_ref()
        .map_or(&[][..], |extras| extras.target_names.as_slice());
    let mut merged = Merged::default();
    let mut shared: Vec<Shared> = Vec::new();

    for (index, primitive) in described.primitives.iter().enumerate() {
        ctx.checkpoint()?;
        let what = format!("primitive {index} of mesh \"{name}\"");
        if !matches!(primitive.mode, TRIANGLES | TRIANGLE_STRIP | TRIANGLE_FAN) {
            ctx.warn(Warning::Unsupported(format!(
                "skipping {what}: points and lines are not supported"
            )));
            continue;
        }

        if let Some(shared_primitive) = shared.iter().find(|shared_primitive| {
            shared_primitive.streams(described) == (&primitive.attributes, &primitive.targets)
        }) {
            let Some(indices) =
                read_indices(accessors, primitive, shared_primitive.count, &what, ctx)?
            else {
                continue;
            };
            read.grow(0, indices.len(), ctx)?;
            add_submesh(
                &mut merged,
                &indices,
                shared_primitive.base,
                material_of(primitive, materials),
            );
            continue;
        }

        let Some(piece) = read_piece(accessors, primitive, &what, materials, ctx)? else {
            continue;
        };
        let base = merged.vertices.len() as u32;
        let count = piece.vertices.len();
        let reusable = !piece.unshared;
        read.grow(count, piece.indices.len(), ctx)?;
        ctx.charge(morph_bytes(&piece))?;
        append(&mut merged, piece, names);
        if reusable {
            shared.push(Shared {
                primitive: index,
                base,
                count,
            });
        }
    }

    if merged.vertices.is_empty() || merged.indices.is_empty() {
        return Ok(None);
    }
    for (target, shaped) in merged.targets.iter_mut().zip(&merged.shaped) {
        if !shaped {
            target.normals.clear();
        }
    }
    Ok(Some(Mesh {
        name: name.to_owned(),
        vertices: merged.vertices,
        indices: merged.indices,
        submeshes: merged.submeshes,
        morph_targets: merged.targets,
        polygons: None,
    }))
}

fn morph_bytes(piece: &Piece) -> u64 {
    let deltas: usize = piece
        .targets
        .iter()
        .map(|target| target.positions.len() + target.normals.len())
        .sum();
    (deltas as u64).saturating_mul(size_of::<Vec3>() as u64)
}

fn read_piece(
    accessors: &Accessors<'_>,
    primitive: &document::Primitive,
    what: &str,
    materials: usize,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<Piece>> {
    let Some(vertices) = attributes(accessors, primitive, what, ctx)? else {
        return Ok(None);
    };
    let count = vertices.len();
    let Some(indices) = read_indices(accessors, primitive, count, what, ctx)? else {
        return Ok(None);
    };

    let mut piece = Piece {
        vertices,
        indices,
        targets: targets(accessors, primitive, count, what, ctx)?,
        material: material_of(primitive, materials),
        unshared: false,
    };
    if piece.vertices.normals.is_empty() {
        unshare(&mut piece);
        piece.vertices.normals = flat_normals(&piece.vertices.positions, &piece.indices);
        piece.vertices.tangents.clear();
        piece.unshared = true;
    }
    Ok(Some(piece))
}

fn material_of(primitive: &document::Primitive, materials: usize) -> Option<MaterialId> {
    primitive
        .material
        .filter(|material| (*material as usize) < materials)
        .map(MaterialId)
}

fn read_indices(
    accessors: &Accessors<'_>,
    primitive: &document::Primitive,
    count: usize,
    what: &str,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<Vec<u32>>> {
    let listed = match primitive.indices {
        Some(indices) => {
            let read = accessors.integers(indices)?;
            if let Some(&worst) = read.iter().max()
                && worst as usize >= count
            {
                return Err(Error::Damaged(format!(
                    "{what} index {worst} out of range for {count} vertices"
                )));
            }
            read
        }
        None => (0..count as u32).collect(),
    };
    Ok(triangulate(primitive.mode, listed, what, ctx))
}

fn attributes(
    accessors: &Accessors<'_>,
    primitive: &document::Primitive,
    what: &str,
    ctx: &mut ImportCtx<'_>,
) -> Result<Option<Vertices>> {
    let Some(&positions) = primitive.attributes.get("POSITION") else {
        ctx.warn(Warning::Unsupported(format!(
            "skipping {what} without POSITION"
        )));
        return Ok(None);
    };
    let mut vertices = Vertices {
        positions: accessors.vec3(positions)?,
        ..Vertices::default()
    };
    let count = vertices.positions.len();
    if count == 0 {
        return Ok(None);
    }

    let accessor = |name: &str| primitive.attributes.get(name).copied();
    if let Some(normals) = accessor("NORMAL") {
        vertices.normals = accessors.vec3(normals)?;
    }
    if let Some(tangents) = accessor("TANGENT") {
        vertices.tangents = accessors.vec4(tangents)?;
    }
    if let Some(uvs) = accessor("TEXCOORD_0") {
        vertices.uvs = accessors.vec2(uvs)?;
    }
    if let Some(colors) = accessor("COLOR_0") {
        vertices.colors = read_colors(accessors, colors)?;
    }
    if let Some(joints) = accessor("JOINTS_0") {
        vertices.joints = read_joints(accessors, joints)?;
    }
    if let Some(weights) = accessor("WEIGHTS_0") {
        vertices.weights = accessors.vec4(weights)?;
    }
    for extra in extras(&primitive.attributes) {
        ctx.warn(Warning::Unsupported(format!(
            "ignoring attribute \"{extra}\" of {what}"
        )));
    }

    for (attribute, actual) in [
        ("NORMAL", vertices.normals.len()),
        ("TANGENT", vertices.tangents.len()),
        ("TEXCOORD_0", vertices.uvs.len()),
        ("COLOR_0", vertices.colors.len()),
        ("JOINTS_0", vertices.joints.len()),
        ("WEIGHTS_0", vertices.weights.len()),
    ] {
        if actual != 0 && actual != count {
            return Err(Error::Damaged(format!(
                "{what}: \"{attribute}\" has {actual} elements for {count} vertices"
            )));
        }
    }
    if vertices.joints.is_empty() != vertices.weights.is_empty() {
        vertices.joints.clear();
        vertices.weights.clear();
        ctx.warn(Warning::Repaired(format!(
            "{what} has JOINTS without WEIGHTS, dropping skinning"
        )));
    }
    Ok(Some(vertices))
}

fn extras(attributes: &BTreeMap<String, Index>) -> Vec<&str> {
    attributes
        .keys()
        .map(String::as_str)
        .filter(|name| {
            !name.starts_with('_')
                && !matches!(
                    *name,
                    "POSITION"
                        | "NORMAL"
                        | "TANGENT"
                        | "TEXCOORD_0"
                        | "COLOR_0"
                        | "JOINTS_0"
                        | "WEIGHTS_0"
                )
        })
        .collect()
}

fn read_colors(accessors: &Accessors<'_>, index: Index) -> Result<Vec<[u8; 4]>> {
    let shape = accessors.shape(index)?;
    let components = match shape {
        Shape::Vec3 | Shape::Vec4 => shape.components(),
        other => {
            return Err(Error::Damaged(format!(
                "accessor {index}: {other:?} is not a vertex colour type"
            )));
        }
    };
    Ok(accessors
        .floats(index)?
        .chunks_exact(components)
        .map(|color| {
            let byte = |slot: usize| {
                (color.get(slot).copied().unwrap_or(1.0).clamp(0.0, 1.0) * 255.0).round() as u8
            };
            [byte(0), byte(1), byte(2), byte(3)]
        })
        .collect())
}

fn read_joints(accessors: &Accessors<'_>, index: Index) -> Result<Vec<[u16; INFLUENCES]>> {
    if accessors.shape(index)? != Shape::Vec4 {
        return Err(Error::Damaged(format!(
            "accessor {index}: joints must be VEC4"
        )));
    }
    accessors
        .integers(index)?
        .as_chunks::<INFLUENCES>()
        .0
        .iter()
        .map(|chunk| {
            let mut joints = [0; INFLUENCES];
            for (slot, joint) in chunk.iter().enumerate() {
                joints[slot] = u16::try_from(*joint).map_err(|_| {
                    Error::Damaged(format!(
                        "accessor {index}: joint {joint} exceeds the {} joint limit",
                        u16::MAX
                    ))
                })?;
            }
            Ok(joints)
        })
        .collect()
}

fn targets(
    accessors: &Accessors<'_>,
    primitive: &document::Primitive,
    count: usize,
    what: &str,
    ctx: &mut ImportCtx<'_>,
) -> Result<Vec<Deltas>> {
    let mut targets = Vec::with_capacity(primitive.targets.len());
    let mut dropped = false;
    for target in &primitive.targets {
        let mut deltas = Deltas::default();
        if let Some(&positions) = target.get("POSITION") {
            deltas.positions = accessors.vec3(positions)?;
        }
        if let Some(&normals) = target.get("NORMAL") {
            deltas.normals = accessors.vec3(normals)?;
        }
        dropped |= target.contains_key("TANGENT");
        for stream in [&deltas.positions, &deltas.normals] {
            if !stream.is_empty() && stream.len() != count {
                return Err(Error::Damaged(format!(
                    "morph target of {what} has {} of {count} vertices",
                    stream.len()
                )));
            }
        }
        targets.push(deltas);
    }
    if dropped {
        ctx.warn(Warning::Unsupported(format!(
            "ignoring tangent deltas in morph target of {what}"
        )));
    }
    Ok(targets)
}

fn triangulate(
    mode: u32,
    indices: Vec<u32>,
    what: &str,
    ctx: &mut ImportCtx<'_>,
) -> Option<Vec<u32>> {
    match mode {
        TRIANGLES => {
            let whole = indices.len() - indices.len() % 3;
            if whole != indices.len() {
                ctx.warn(Warning::Repaired(format!(
                    "{what} has a trailing partial triangle, dropping it"
                )));
            }
            (whole > 0).then(|| indices[..whole].to_vec())
        }
        TRIANGLE_STRIP => {
            let triangles: Vec<u32> = indices
                .windows(3)
                .enumerate()
                .flat_map(|(step, corner)| match step % 2 {
                    0 => [corner[0], corner[1], corner[2]],
                    _ => [corner[0], corner[2], corner[1]],
                })
                .collect();
            (!triangles.is_empty()).then_some(triangles)
        }
        TRIANGLE_FAN => {
            let triangles: Vec<u32> = indices
                .windows(2)
                .skip(1)
                .flat_map(|corner| [indices[0], corner[0], corner[1]])
                .collect();
            (!triangles.is_empty()).then_some(triangles)
        }
        _ => None,
    }
}

fn unshare(piece: &mut Piece) {
    let order = std::mem::take(&mut piece.indices);
    piece.vertices = gather(&piece.vertices, &order);
    for target in &mut piece.targets {
        target.positions = pick(&target.positions, &order);
        target.normals = pick(&target.normals, &order);
    }
    piece.indices = (0..order.len() as u32).collect();
}

fn gather(vertices: &Vertices, order: &[u32]) -> Vertices {
    Vertices {
        positions: pick(&vertices.positions, order),
        normals: pick(&vertices.normals, order),
        tangents: pick(&vertices.tangents, order),
        uvs: pick(&vertices.uvs, order),
        colors: pick(&vertices.colors, order),
        joints: pick(&vertices.joints, order),
        weights: pick(&vertices.weights, order),
    }
}

fn pick<T: Copy>(values: &[T], order: &[u32]) -> Vec<T> {
    match values.is_empty() {
        true => Vec::new(),
        false => order.iter().map(|&at| values[at as usize]).collect(),
    }
}

fn flat_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut normals = vec![Vec3::Y; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let corner = |slot: usize| positions[triangle[slot] as usize];
        let face = (corner(1) - corner(0))
            .cross(corner(2) - corner(0))
            .normalize_or(Vec3::Y);
        for &index in triangle {
            normals[index as usize] = face;
        }
    }
    normals
}

fn add_submesh(merged: &mut Merged, indices: &[u32], base: u32, material: Option<MaterialId>) {
    let first_index = merged.indices.len();
    merged
        .indices
        .extend(indices.iter().map(|index| index + base));
    merged.submeshes.push(Submesh {
        name: String::new(),
        first_index: first_index as u32,
        index_count: (merged.indices.len() - first_index) as u32,
        material,
        bounds: Bounds::EMPTY,
    });
}

fn append(merged: &mut Merged, piece: Piece, names: &[String]) {
    let base = merged.vertices.len();
    let added = piece.vertices.len();

    add_submesh(merged, &piece.indices, base as u32, piece.material);
    extend(&mut merged.vertices, &piece.vertices);

    while merged.targets.len() < piece.targets.len() {
        let slot = merged.targets.len();
        merged.targets.push(MorphTarget::new(
            names.get(slot).cloned().unwrap_or_default(),
        ));
        merged.shaped.push(false);
    }
    for (slot, deltas) in piece.targets.iter().enumerate() {
        let target = &mut merged.targets[slot];
        merged.shaped[slot] |= !deltas.normals.is_empty();
        for vertex in 0..added {
            let position = deltas.positions.get(vertex).copied().unwrap_or(Vec3::ZERO);
            let normal = deltas.normals.get(vertex).copied().unwrap_or(Vec3::ZERO);
            if position != Vec3::ZERO || normal != Vec3::ZERO {
                target.push((base + vertex) as u32, position, Some(normal));
            }
        }
    }
}

fn extend(into: &mut Vertices, from: &Vertices) {
    let before = into.len();
    let added = from.len();
    fill(&mut into.normals, &from.normals, before, added, Vec3::Y);
    fill(
        &mut into.tangents,
        &from.tangents,
        before,
        added,
        Vec4::new(1.0, 0.0, 0.0, 1.0),
    );
    fill(&mut into.uvs, &from.uvs, before, added, Vec2::ZERO);
    fill(&mut into.colors, &from.colors, before, added, [255; 4]);
    fill(
        &mut into.joints,
        &from.joints,
        before,
        added,
        [0; INFLUENCES],
    );
    fill(&mut into.weights, &from.weights, before, added, Vec4::ZERO);
    into.positions.extend_from_slice(&from.positions);
}

fn fill<T: Copy>(into: &mut Vec<T>, from: &[T], before: usize, added: usize, default: T) {
    if into.is_empty() && from.is_empty() {
        return;
    }
    into.resize(before, default);
    match from.is_empty() {
        true => into.resize(before + added, default),
        false => into.extend_from_slice(from),
    }
}
