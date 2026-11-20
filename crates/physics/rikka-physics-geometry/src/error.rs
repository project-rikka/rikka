#[derive(Debug, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("convex hull needs at least 4 non-coplanar points, got {points}")]
    DegenerateHull { points: usize },
    #[error("convex hull supports at most 255 vertices, got {requested}")]
    HullTooLarge { requested: usize },
    #[error("mesh index {index} out of range for {vertices} vertices")]
    IndexOutOfRange { index: u32, vertices: usize },
    #[error("mesh needs three indices per triangle, got {count}")]
    IndexCount { count: usize },
    #[error("mesh has no non-degenerate triangle")]
    EmptyMesh,
    #[error("expected {expected} material indices, got {actual}")]
    MaterialCount { expected: usize, actual: usize },
    #[error("{shape} data is inconsistent")]
    CorruptShape { shape: &'static str },
    #[error("height field needs at least 2 x 2 samples, got {rows} x {columns}")]
    HeightFieldTooSmall { rows: u32, columns: u32 },
    #[error("height field has {actual} samples, expected {expected}")]
    HeightFieldSize { expected: usize, actual: usize },
    #[error("compound needs at least one child")]
    EmptyCompound,
    #[error("compound cannot contain a {kind:?} child")]
    UnsupportedChild { kind: crate::ShapeKind },
    #[error("{what} must be finite and positive, got {value}")]
    InvalidDimension { what: &'static str, value: f32 },
    #[error("{what} must be finite and non-negative, got {value}")]
    NegativeRadius { what: &'static str, value: f32 },
    #[error("{what} must be finite")]
    NonFinite { what: &'static str },
    #[error("{what} must be a unit quaternion, got length {length}")]
    UnnormalizedRotation { what: &'static str, length: f32 },
    #[error("{what} must be a unit vector, got length {length}")]
    NotUnitVector { what: &'static str, length: f32 },
    #[error(transparent)]
    Blob(#[from] rikka_physics_core::BlobError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
