mod array;
mod bevel;
mod bisect;
mod boolean;
mod corrective_smooth;
mod data_transfer;
mod decimate;
mod displace;
mod edge_split;
mod editing;
pub(crate) mod lattice;
mod mirror;
mod shrinkwrap;
mod smooth_by_angle;
mod solidify;
mod subdivision;
mod triangulate;
mod vertex_group;
mod weighted_normal;
mod weld;

use std::sync::Arc;

use glam::Vec3;
use rikka_polymesh::Polymesh;

use crate::context::Context;
use crate::kinds::boolean::{BooleanOperand, BooleanSolver};
use crate::kinds::displace::DisplaceDirection;
use crate::modifier::{ModifierKind, Purpose};
use crate::note::{NoteKind, SkipReason};
use crate::reference::MaterialRef;

pub(crate) use subdivision::limit_positions;

pub(crate) struct Scope<'a> {
    pub(crate) ctx: &'a dyn Context,
    pub(crate) purpose: Purpose,
    pub(crate) original: &'a [Vec3],
    material_slots: Option<Vec<Option<MaterialRef>>>,
    notes: Vec<NoteKind>,
}

impl<'a> Scope<'a> {
    pub(crate) fn new(ctx: &'a dyn Context, purpose: Purpose, original: &'a [Vec3]) -> Scope<'a> {
        Scope {
            ctx,
            purpose,
            original,
            material_slots: None,
            notes: Vec::new(),
        }
    }

    pub(crate) fn with_material_slots(
        mut self,
        slots: Option<Vec<Option<MaterialRef>>>,
    ) -> Scope<'a> {
        self.material_slots = slots;
        self
    }

    pub(crate) fn material_slots(&self) -> Vec<Option<MaterialRef>> {
        self.material_slots
            .clone()
            .unwrap_or_else(|| self.ctx.materials())
    }

    pub(crate) fn note(&mut self, kind: NoteKind) {
        if !self.notes.contains(&kind) {
            self.notes.push(kind);
        }
    }

    pub(crate) fn into_notes(self) -> Vec<NoteKind> {
        self.notes
    }
}

pub(crate) fn target(
    ctx: &dyn Context,
    object: crate::reference::ObjectRef,
) -> Result<std::sync::Arc<crate::target::Target>, SkipReason> {
    ctx.target(object).ok_or_else(|| match ctx.mesh(object) {
        Some(_) => SkipReason::InvalidTarget,
        None => SkipReason::NotAMesh,
    })
}

pub(crate) fn disabled(kind: &ModifierKind, ctx: &dyn Context) -> Option<SkipReason> {
    let referenced = |object: Option<crate::reference::ObjectRef>| match object {
        None => Some(SkipReason::MissingObject),
        Some(object) if Some(object) == ctx.object() => Some(SkipReason::SelfReference),
        Some(_) => None,
    };
    match kind {
        ModifierKind::Displace(displace) => {
            if displace.texture.is_none() && displace.direction == DisplaceDirection::RgbToXyz {
                Some(SkipReason::MissingTexture)
            } else if displace.strength == 0.0 {
                Some(SkipReason::NoStrength)
            } else {
                None
            }
        }
        ModifierKind::Shrinkwrap(shrinkwrap) => referenced(shrinkwrap.target),
        ModifierKind::DataTransfer(transfer) => referenced(transfer.object),
        ModifierKind::Armature(armature) => referenced(armature.object),
        ModifierKind::Boolean(boolean) => match boolean.operand_type {
            BooleanOperand::Object => referenced(boolean.object),
            BooleanOperand::Collection => (boolean.collection.is_none()
                && boolean.solver != BooleanSolver::Exact)
                .then_some(SkipReason::MissingObject),
        },
        ModifierKind::Bevel(bevel) => (bevel.width == 0.0).then_some(SkipReason::NoWidth),
        _ => None,
    }
}

#[derive(Clone, Default)]
pub(crate) struct Slot {
    subdivision: Option<Arc<subdivision::Cached>>,
}

pub(crate) fn run(
    kind: &ModifierKind,
    mesh: &mut Polymesh,
    scope: &mut Scope<'_>,
    slot: Option<&mut Slot>,
) -> Result<(), SkipReason> {
    if let Some(reason) = disabled(kind, scope.ctx) {
        return Err(reason);
    }
    match kind {
        ModifierKind::Mirror(mirror) => mirror::evaluate(mirror, mesh, scope),
        ModifierKind::Subdivision(subdivision) => {
            let cached = slot.map(|slot| &mut slot.subdivision);
            subdivision::evaluate(subdivision, mesh, scope, cached);
            Ok(())
        }
        ModifierKind::Solidify(solidify) => solidify::evaluate(solidify, mesh, scope),
        ModifierKind::Array(array) => {
            array::evaluate(array, mesh, scope);
            Ok(())
        }
        ModifierKind::Weld(weld) => {
            weld::evaluate(weld, mesh);
            Ok(())
        }
        ModifierKind::Triangulate(triangulate) => {
            triangulate::evaluate(triangulate, mesh);
            Ok(())
        }
        ModifierKind::EdgeSplit(edge_split) => edge_split::evaluate(edge_split, mesh),
        ModifierKind::Displace(displace) => {
            deform(mesh, |mesh| displace::evaluate(displace, mesh, scope))
        }
        ModifierKind::Shrinkwrap(shrinkwrap) => {
            deform(mesh, |mesh| shrinkwrap::evaluate(shrinkwrap, mesh, scope))
        }
        ModifierKind::DataTransfer(transfer) => data_transfer::evaluate(transfer, mesh, scope),
        ModifierKind::SmoothByAngle(smooth) => {
            smooth_by_angle::evaluate(smooth, mesh);
            Ok(())
        }
        ModifierKind::Bevel(settings) => bevel::evaluate(settings, mesh, scope),
        ModifierKind::Boolean(settings) => boolean::evaluate(settings, mesh, scope),
        ModifierKind::Decimate(settings) => decimate::evaluate(settings, mesh, scope),
        ModifierKind::WeightedNormal(settings) => {
            weighted_normal::evaluate(settings, mesh);
            Ok(())
        }
        ModifierKind::CorrectiveSmooth(settings) => {
            let original = scope.original;
            deform(mesh, |mesh| {
                corrective_smooth::evaluate(settings, mesh, original)
            })
        }
        ModifierKind::Lattice(settings) => deform(mesh, |mesh| lattice::evaluate(settings, mesh)),
        ModifierKind::Armature(_) => Err(SkipReason::SceneEvaluated),
    }
}

fn deform(
    mesh: &mut Polymesh,
    evaluate: impl FnOnce(&mut Polymesh) -> Result<(), SkipReason>,
) -> Result<(), SkipReason> {
    if mesh.shape_keys.is_empty() {
        return evaluate(mesh);
    }
    let before = mesh.positions.clone();
    evaluate(mesh)?;
    let after = mesh.positions.clone();
    offset_shape_keys(mesh, &before, &after);
    Ok(())
}

pub(crate) fn offset_shape_keys(mesh: &mut Polymesh, before: &[Vec3], after: &[Vec3]) {
    for key in &mut mesh.shape_keys.keys {
        for ((position, old), new) in key.positions.iter_mut().zip(before).zip(after) {
            *position += *new - *old;
        }
    }
}
