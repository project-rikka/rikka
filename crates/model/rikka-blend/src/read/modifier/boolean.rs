use rikka_mesh_modifiers::{
    Boolean, BooleanMaterialMode, BooleanOperand, BooleanOperation, BooleanSolver,
};

use super::{References, Targets};
use crate::file::Record;
use crate::write::Struct;

const SELF: i64 = 1;
const OPERAND_OBJECT: i64 = 1 << 1;
const OPERAND_COLLECTION: i64 = 1 << 2;
const HOLE_TOLERANT: i64 = 1 << 3;
const OPERATIONS: [BooleanOperation; 3] = [
    BooleanOperation::Intersect,
    BooleanOperation::Union,
    BooleanOperation::Difference,
];
const SOLVERS: [BooleanSolver; 3] = [
    BooleanSolver::Fast,
    BooleanSolver::Exact,
    BooleanSolver::Manifold,
];

pub(super) fn read<'a>(modifier: &Record<'a>, refs: &dyn References<'a>) -> Boolean {
    let flag = modifier.int("flag").unwrap_or(OPERAND_OBJECT);
    let pick =
        |name: &str, default: i64| usize::try_from(modifier.int(name).unwrap_or(default)).ok();
    Boolean {
        operation: pick("operation", 2)
            .and_then(|index| OPERATIONS.get(index).copied())
            .unwrap_or_default(),
        operand_type: match flag & OPERAND_COLLECTION != 0 {
            true => BooleanOperand::Collection,
            false => BooleanOperand::Object,
        },
        object: refs.object(modifier.id("object")),
        collection: refs.collection(modifier.id("collection")),
        solver: pick("solver", 1)
            .and_then(|index| SOLVERS.get(index).copied())
            .unwrap_or_default(),
        double_threshold: modifier
            .float("double_threshold")
            .unwrap_or(1.0e-6)
            .max(0.0),
        use_self: flag & SELF != 0,
        use_hole_tolerant: flag & HOLE_TOLERANT != 0,
        material_mode: match modifier.int("material_mode").unwrap_or(0) {
            1 => BooleanMaterialMode::Transfer,
            _ => BooleanMaterialMode::Index,
        },
    }
}

pub(super) fn write(boolean: &Boolean, record: &mut Struct<'_>, targets: &dyn Targets) {
    let collection = boolean.operand_type == BooleanOperand::Collection;
    record
        .set_int(
            "operation",
            OPERATIONS
                .iter()
                .position(|operation| *operation == boolean.operation)
                .unwrap_or(2) as i64,
        )
        .set_flag("flag", OPERAND_OBJECT, !collection)
        .set_flag("flag", OPERAND_COLLECTION, collection)
        .set_pointer("object", targets.object(boolean.object))
        .set_pointer("collection", targets.collection(boolean.collection))
        .set_int(
            "solver",
            SOLVERS
                .iter()
                .position(|solver| *solver == boolean.solver)
                .unwrap_or(1) as i64,
        )
        .set_float("double_threshold", boolean.double_threshold)
        .set_flag("flag", SELF, boolean.use_self)
        .set_flag("flag", HOLE_TOLERANT, boolean.use_hole_tolerant)
        .set_int(
            "material_mode",
            match boolean.material_mode {
                BooleanMaterialMode::Index => 0,
                BooleanMaterialMode::Transfer => 1,
            },
        );
}
