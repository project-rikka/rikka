use rikka_slang::SchemaError;

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error(transparent)]
    Slang(#[from] rikka_slang::Error),
    #[error("invalid material parameters: {0}")]
    Schema(#[from] SchemaError),
    #[error("material methods {} copy their parameters on every call; mark them [constref]", .0.join(", "))]
    Copied(Vec<String>),
    #[error(
        "the vertex hook moves vertices without a [Role(\"displacement\")] float parameter bounding how far; \
         declare one (a negative value means unbounded)"
    )]
    Unbounded,
    #[error("effect entry `{0}` is a vertex shader")]
    VertexEffect(String),
}
