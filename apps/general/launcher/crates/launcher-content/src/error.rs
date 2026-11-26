#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Modrinth(#[from] launcher_modrinth::Error),
    #[error(transparent)]
    Instance(#[from] launcher_instance::Error),
    #[error("failed to download {file}: {source}")]
    Download {
        file: String,
        #[source]
        source: rikka_net::Error,
    },
    #[error("{0} has no version for this Minecraft release")]
    NoCompatibleVersion(String),
    #[error("{0} cannot be installed here")]
    Unsupported(String),
    #[error("{project} cannot be used together with {installed}")]
    Incompatible { project: String, installed: String },
    #[error("version {0} has no file")]
    NoFile(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
