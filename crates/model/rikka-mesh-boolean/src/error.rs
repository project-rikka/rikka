#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("vertex {index} has a non-finite coordinate")]
    NonFiniteVertex { index: usize },

    #[error("vertex index {index} out of range for {len} vertices")]
    VertexIndex { index: u32, len: usize },

    #[error("face {face} has fewer than three vertices")]
    FaceTooSmall { face: usize },

    #[error("epsilon {0} is not a finite non-negative distance")]
    InvalidEpsilon(f64),

    #[error("boolean needs at least one operand")]
    NoOperands,

    #[error("operand {operand} is not a valid mesh: {error}")]
    InvalidOperand {
        operand: usize,
        error: rikka_polymesh::Error,
    },

    #[error("operand {operand} transform is not finite")]
    NonFiniteTransform { operand: usize },

    #[error("threshold {0} is not a finite non-negative distance")]
    InvalidThreshold(f32),

    #[error(transparent)]
    Edit(#[from] rikka_mesh_edit::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
