use std::sync::Arc;

use glam::Mat4;
use rikka_mesh_boolean::{
    Boolean as Solve, MaterialMode, Operand, Operation, Solver, material_remap,
};
use rikka_polymesh::{Domain, Polymesh};

use super::Scope;
use crate::kinds::boolean::{
    Boolean, BooleanMaterialMode, BooleanOperand, BooleanOperation, BooleanSolver,
};
use crate::note::{NoteKind, SkipReason};
use crate::origins;
use crate::reference::{MaterialRef, ObjectRef};
use crate::space::inverse;

struct Source {
    object: ObjectRef,
    mesh: Arc<Polymesh>,
    transform: Mat4,
}

type Step<'a> = (&'a Source, Option<Vec<i32>>);

pub(crate) fn evaluate(
    settings: &Boolean,
    mesh: &mut Polymesh,
    scope: &mut Scope<'_>,
) -> Result<(), SkipReason> {
    let solver = match settings.solver {
        BooleanSolver::Fast => Solver::Fast,
        BooleanSolver::Exact => Solver::Exact,
        BooleanSolver::Manifold => {
            scope.note(NoteKind::ManifoldSolverAsExact);
            Solver::Exact
        }
    };
    let sources = sources(settings, scope)?;
    let operation = match settings.operation {
        BooleanOperation::Intersect => Operation::Intersect,
        BooleanOperation::Union => Operation::Union,
        BooleanOperation::Difference => Operation::Difference,
    };
    let ctx = scope.ctx;
    let own = ctx.materials();
    let mut slots = scope.material_slots();
    let transfer =
        solver == Solver::Exact && settings.material_mode == BooleanMaterialMode::Transfer;
    if transfer && slots.is_empty() {
        slots.push(None);
    }
    let remaps: Vec<Option<Vec<i32>>> = sources
        .iter()
        .map(|source| {
            let operand = ctx.object_materials(source.object)?;
            Some(match transfer {
                true => material_remap(MaterialMode::Transfer, &mut slots, &operand),
                false => index_remap(&own, &operand),
            })
        })
        .collect();

    let mut working = mesh.clone();
    let mut face_origins = origins::values(&working, Domain::Face);
    origins::seal(&mut working);
    let solve = |operands: Vec<Operand<'_>>| {
        Solve::new(operation)
            .solver(solver)
            .operands(operands)
            .self_intersection(settings.use_self)
            .hole_tolerant(settings.use_hole_tolerant)
            .threshold(settings.double_threshold)
            .compute()
            .map_err(|_| SkipReason::InvalidMesh)
    };
    let steps: Vec<Vec<Step<'_>>> = match solver {
        Solver::Exact => vec![sources.iter().zip(remaps).collect()],
        Solver::Fast => sources.iter().zip(remaps).map(|step| vec![step]).collect(),
    };
    for step in steps {
        let mut operands = vec![Operand::new(&working)];
        operands.extend(
            step.into_iter()
                .map(|(source, remap)| operand(source, remap)),
        );
        let computed = solve(operands)?;
        face_origins = face_origins.map(|input| {
            computed
                .face_origins
                .iter()
                .map(|origin| match origin.operand {
                    0 => input.get(origin.face as usize).copied().unwrap_or(0),
                    _ => 0,
                })
                .collect()
        });
        working = computed.mesh;
    }
    origins::unseal(&mut working);
    if let Some(values) = face_origins {
        origins::assign(&mut working, Domain::Face, values);
    }
    *mesh = working;
    if transfer {
        scope.note(NoteKind::MaterialSlots(slots));
    }
    Ok(())
}

fn index_remap(target: &[Option<MaterialRef>], operand: &[Option<MaterialRef>]) -> Vec<i32> {
    let mut target = target.to_vec();
    material_remap(MaterialMode::IndexBased, &mut target, operand)
}

fn operand(source: &Source, remap: Option<Vec<i32>>) -> Operand<'_> {
    let operand = Operand::new(source.mesh.as_ref()).transform(source.transform);
    match remap {
        Some(remap) => operand.material_remap(remap),
        None => operand,
    }
}

fn sources(settings: &Boolean, scope: &Scope<'_>) -> Result<Vec<Source>, SkipReason> {
    let ctx = scope.ctx;
    let world_to_object = inverse(ctx.object_matrix());
    let source = |object: ObjectRef, mesh: Arc<Polymesh>| Source {
        object,
        mesh,
        transform: world_to_object * ctx.matrix(object).unwrap_or(Mat4::IDENTITY),
    };
    match settings.operand_type {
        BooleanOperand::Object => {
            let Some(object) = settings.object else {
                return Err(SkipReason::MissingObject);
            };
            let mesh = ctx.mesh(object).ok_or(SkipReason::NotAMesh)?;
            Ok(vec![source(object, mesh)])
        }
        BooleanOperand::Collection => {
            let members = settings
                .collection
                .and_then(|collection| ctx.collection_objects(collection))
                .unwrap_or_default();
            let mut failure = None;
            if settings.operation == BooleanOperation::Intersect
                && settings.solver == BooleanSolver::Fast
            {
                failure = Some(SkipReason::IntersectNeedsExact);
            }
            if settings.solver != BooleanSolver::Exact && members.is_empty() {
                failure = Some(SkipReason::EmptyCollection);
            }
            let mut sources = Vec::new();
            for object in members {
                if Some(object) == ctx.object() {
                    continue;
                }
                match ctx.mesh(object) {
                    Some(mesh) => sources.push(source(object, mesh)),
                    None => failure = Some(SkipReason::NonMeshInCollection),
                }
            }
            match failure {
                Some(reason) => Err(reason),
                None => Ok(sources),
            }
        }
    }
}
