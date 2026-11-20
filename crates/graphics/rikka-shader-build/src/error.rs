use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to run {compiler}: {source}")]
    Spawn {
        compiler: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to compile {shader}:\n{report}")]
    Compile { shader: PathBuf, report: String },
    #[error("{first} and {second} both map to constant {constant}")]
    ConstantCollision {
        constant: String,
        first: PathBuf,
        second: PathBuf,
    },
    #[error("{0} declares no [shader(..)] entry point; Slang modules belong under lib/")]
    MissingEntryPoint(PathBuf),
    #[error("{0} is a library module under lib/ but declares a [shader(..)] entry point")]
    LibraryEntryPoint(PathBuf),
    #[error("{shader} declares unsupported stage \"{stage}\"")]
    UnknownStage { shader: PathBuf, stage: String },
    #[error("{shader} declares a {stage} entry point without a function name")]
    UnnamedEntryPoint {
        shader: PathBuf,
        stage: &'static str,
    },
    #[error("{path} is not SPIR-V: {source}")]
    Spirv {
        path: PathBuf,
        source: rikka_spirv::Error,
    },
    #[error("{0} lists no dependencies")]
    Dependencies(PathBuf),
    #[error("{0}")]
    Usage(String),
    #[error("{0} has a vertex entry point without an SV_Position output")]
    MissingPosition(PathBuf),
    #[error("{shader} declares two {stage} entry points: {first} and {second}")]
    DuplicateStage {
        shader: PathBuf,
        stage: &'static str,
        first: String,
        second: String,
    },
}
