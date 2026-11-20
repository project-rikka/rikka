pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("libslang is unavailable: {0}")]
    Unavailable(String),
    #[error("failed to compile shader: {diagnostics}")]
    Compile { diagnostics: String },
    #[error("shader has no entry point `{0}`")]
    MissingEntryPoint(String),
    #[error("shader has no type `{0}`")]
    UnknownType(String),
    #[error("entry point `{name}` has unsupported stage {stage}")]
    UnsupportedStage { name: String, stage: u32 },
    #[error("{call} failed with {code:#x}")]
    Call { call: &'static str, code: i32 },
    #[error("{0} contains a NUL byte")]
    Nul(&'static str),
    #[error("failed to inline SPIR-V calls: {0}")]
    Flatten(String),
}
