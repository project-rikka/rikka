use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Meta(#[from] launcher_meta::Error),
    #[error("failed to download {url}: {source}")]
    Download {
        url: String,
        #[source]
        source: rikka_net::Error,
    },
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Mojang does not list Minecraft {0}")]
    UnknownVersion(String),
    #[error("{path} does not match its checksum")]
    Corrupt { path: PathBuf },
    #[error("no Java runtime is installed beside the launcher")]
    NoBundledJava,
    #[error("Mojang has no Java runtime {component} for {platform}")]
    NoRuntime {
        component: String,
        platform: &'static str,
    },
    #[error("{0} is not a Java runtime")]
    NotJava(PathBuf),
    #[error("Minecraft needs Java {required}, but {path} is Java {found}")]
    OldJava {
        path: PathBuf,
        required: u32,
        found: u32,
    },
    #[error("the Java runtime manifest links {0}, which this platform does not support")]
    Link(String),
    #[error("the installation was cancelled")]
    Cancelled,
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.into();
        move |source| Error::Io { path, source }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
