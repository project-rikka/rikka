use glam::Vec3;
use rikka_polymesh::{Polymesh, ShapeKey};

use crate::context::Context;
use crate::error::{Error, Result};
use crate::evaluators::{self, Scope};
use crate::modifier::{Modifier, Purpose};
use crate::note::NoteKind;
use crate::origins;

pub fn apply(mesh: &mut Polymesh, modifier: &Modifier, ctx: &dyn Context) -> Result<Vec<NoteKind>> {
    let name = || modifier.name.clone();
    if !modifier.show_viewport || evaluators::disabled(&modifier.kind, ctx).is_some() {
        return Err(Error::Disabled { name: name() });
    }
    let kind = modifier.modifier_type();
    if !mesh.shape_keys.is_empty() && !kind.is_non_geometrical() {
        return Err(Error::ShapeKeys { name: name() });
    }
    let mut result = mesh.clone();
    let notes = run(&mut result, modifier, ctx)?;
    if result.uv_maps.active.is_none() && !result.uv_maps.is_empty() {
        result.uv_maps.active = Some(0);
    }
    *mesh = result;
    Ok(notes)
}

pub fn apply_as_shape_key(
    mesh: &mut Polymesh,
    modifier: &Modifier,
    ctx: &dyn Context,
) -> Result<u32> {
    let name = || modifier.name.clone();
    if evaluators::disabled(&modifier.kind, ctx).is_some() {
        return Err(Error::Disabled { name: name() });
    }
    let kind = modifier.modifier_type();
    if !kind.is_deform_only() || kind.is_non_geometrical() {
        return Err(Error::NotDeforming { name: name() });
    }
    if !modifier.show_viewport {
        return Err(Error::Disabled { name: name() });
    }
    let mut deformed = mesh.clone();
    deformed.positions = mixed_positions(mesh);
    deformed.shape_keys = Default::default();
    run(&mut deformed, modifier, ctx)?;
    if mesh.shape_keys.is_empty() {
        mesh.shape_keys
            .add(ShapeKey::new("Basis", mesh.positions.clone()));
    }
    Ok(mesh
        .shape_keys
        .add(ShapeKey::new(modifier.name.clone(), deformed.positions)))
}

fn run(mesh: &mut Polymesh, modifier: &Modifier, ctx: &dyn Context) -> Result<Vec<NoteKind>> {
    let original = match ctx.original_positions() {
        Some(positions) => positions.to_vec(),
        None => mesh.positions.clone(),
    };
    let mut scope = Scope::new(ctx, Purpose::Viewport, &original);
    let outcome = evaluators::run(&modifier.kind, mesh, &mut scope, None);
    origins::strip(mesh);
    match outcome {
        Ok(()) => Ok(scope.into_notes()),
        Err(reason) => Err(Error::Skipped {
            name: modifier.name.clone(),
            reason,
        }),
    }
}

fn mixed_positions(mesh: &Polymesh) -> Vec<Vec3> {
    let keys = &mesh.shape_keys.keys;
    let Some(basis) = keys.first() else {
        return mesh.positions.clone();
    };
    let mut positions = basis.positions.clone();
    for key in keys.iter().skip(1) {
        if key.muted || key.value == 0.0 {
            continue;
        }
        let Some(reference) = keys.get(key.relative_to as usize) else {
            continue;
        };
        let group = key
            .vertex_group
            .as_deref()
            .and_then(|group| mesh.vertex_groups.index(group));
        for (point, position) in positions.iter_mut().enumerate() {
            let weight = group.map_or(1.0, |group| mesh.vertex_groups.weight(point, group));
            *position += (key.positions[point] - reference.positions[point]) * key.value * weight;
        }
    }
    positions
}
