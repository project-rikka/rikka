#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Requirement {
    #[error("an animation")]
    Animation,

    #[error("avatar data: a humanoid, meta and expressions")]
    Avatar,

    #[error("at least one vertex")]
    Vertices,

    #[error("finite vertex positions, mesh \"{0}\" has others")]
    FiniteVertices(String),

    #[error("at most {allowed} triangles, the model has {actual}")]
    Triangles { actual: u64, allowed: u64 },
}
