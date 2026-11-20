mod complex;
mod simple;

use rikka_polymesh::Polymesh;

use super::Scope;
use crate::kinds::solidify::{Solidify, SolidifyMode};
use crate::note::SkipReason;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaterialOffsets {
    largest: i32,
    shell: i32,
    rim: i32,
}

impl MaterialOffsets {
    fn new(solidify: &Solidify, material_count: usize) -> MaterialOffsets {
        let largest = match material_count > 1 {
            true => material_count.min(i16::MAX as usize) as i32 - 1,
            false => 0,
        };
        let enabled = |offset: i16| match largest {
            0 => 0,
            _ => i32::from(offset),
        };
        MaterialOffsets {
            largest,
            shell: enabled(solidify.material_offset),
            rim: enabled(solidify.material_offset_rim),
        }
    }

    pub(crate) fn shell(&self, material: i32) -> i32 {
        Self::shifted(material, self.shell, self.largest)
    }

    pub(crate) fn rim(&self, material: i32) -> i32 {
        Self::shifted(material, self.rim, self.largest)
    }

    fn shifted(material: i32, offset: i32, largest: i32) -> i32 {
        match offset {
            0 => material,
            offset => (material + offset).clamp(0, largest),
        }
    }

    pub(crate) fn largest(&self) -> i32 {
        self.largest
    }

    pub(crate) fn complex_shell(&self, material: i32, shifted: bool) -> i32 {
        (material + if shifted { self.shell } else { 0 }).clamp(0, self.largest)
    }

    pub(crate) fn complex_rim(&self, material: i32, shifted: bool) -> i32 {
        (material + if shifted { self.rim } else { 0 }).clamp(0, self.largest)
    }
}

pub(crate) fn evaluate(
    solidify: &Solidify,
    mesh: &mut Polymesh,
    scope: &mut Scope<'_>,
) -> Result<(), SkipReason> {
    let materials = MaterialOffsets::new(solidify, scope.ctx.material_count());
    match solidify.solidify_mode {
        SolidifyMode::Simple => {
            simple::evaluate(solidify, mesh, materials);
            Ok(())
        }
        SolidifyMode::Complex => complex::evaluate(solidify, mesh, materials),
    }
}

pub(crate) fn mark_group(mesh: &mut Polymesh, name: &str, points: &[u32]) {
    if name.is_empty() {
        return;
    }
    let Some(group) = mesh.vertex_groups.index(name) else {
        return;
    };
    mesh.vertex_groups.ensure_weights(mesh.positions.len());
    for &point in points {
        mesh.vertex_groups.set_weight(point as usize, group, 1.0);
    }
}
