use crate::codec::{non_negative, record, tagged};
use crate::reference::{CollectionRef, ObjectRef};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BooleanOperation {
    Intersect,
    Union,
    #[default]
    Difference,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BooleanOperand {
    #[default]
    Object,
    Collection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BooleanSolver {
    Fast,
    #[default]
    Exact,
    Manifold,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BooleanMaterialMode {
    #[default]
    Index,
    Transfer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Boolean {
    pub operation: BooleanOperation,
    pub operand_type: BooleanOperand,
    pub object: Option<ObjectRef>,
    pub collection: Option<CollectionRef>,
    pub solver: BooleanSolver,
    pub double_threshold: f32,
    pub use_self: bool,
    pub use_hole_tolerant: bool,
    pub material_mode: BooleanMaterialMode,
}

impl Default for Boolean {
    fn default() -> Boolean {
        Boolean {
            operation: BooleanOperation::Difference,
            operand_type: BooleanOperand::Object,
            object: None,
            collection: None,
            solver: BooleanSolver::Exact,
            double_threshold: 1.0e-6,
            use_self: false,
            use_hole_tolerant: false,
            material_mode: BooleanMaterialMode::Index,
        }
    }
}

tagged!(BooleanOperation, "boolean operation" {
    Intersect = 0,
    Union = 1,
    Difference = 2,
});

tagged!(BooleanOperand, "boolean operand type" {
    Object = 0,
    Collection = 1,
});

tagged!(BooleanSolver, "boolean solver" {
    Fast = 0,
    Exact = 1,
    Manifold = 2,
});

tagged!(BooleanMaterialMode, "boolean material mode" {
    Index = 0,
    Transfer = 1,
});

record!(Boolean {
    operation,
    operand_type,
    object,
    collection,
    solver,
    double_threshold => non_negative("boolean overlap threshold is negative"),
    use_self,
    use_hole_tolerant,
    material_mode,
});
