mod arrangement;
mod boolean;
mod cdt;
mod edit;
mod edit_exact;
mod error;
mod exact;
mod exact_boolean;
mod fast;
mod join;
mod materials;
mod merge;
mod operand;
mod operation;
mod propagate;

pub use boolean::{Boolean, BooleanResult, DEFAULT_THRESHOLD, FaceOrigin};
pub use cdt::{Cdt, CdtOutput, CdtResult, EdgeOrigin};
pub use edit::{IntersectBoolean, IntersectKnife, IntersectSource, SeparateMode};
pub use error::{Error, Result};
pub use materials::{MaterialMode, material_remap};
pub use operand::Operand;
pub use operation::{Operation, Solver};

const PARALLEL: rikka_parallel::Parallel = rikka_parallel::Parallel::new("rikka-mesh-boolean");
