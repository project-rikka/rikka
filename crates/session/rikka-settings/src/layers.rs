use std::io;
use std::path::PathBuf;

use rikka_dirs::Paths;

use crate::atomic;
use crate::domain::Domain;
use crate::error::Result;
use crate::file_cache::FileCache;
use crate::scribe;

#[derive(Clone, Debug)]
pub struct Layers {
    system: Vec<PathBuf>,
    user: PathBuf,
    cache: Option<FileCache>,
}

impl Layers {
    pub fn resolve() -> Result<Self> {
        let paths = Paths::resolve()?;
        rikka_dirs::ensure(paths.config_dir())?;
        Ok(Self {
            system: paths.system_config_dirs().to_vec(),
            user: paths.config_dir().to_path_buf(),
            cache: None,
        })
    }

    pub fn new(system: impl Into<PathBuf>, user: impl Into<PathBuf>) -> Self {
        Self {
            system: vec![system.into()],
            user: user.into(),
            cache: None,
        }
    }

    #[must_use]
    pub fn volatile(self) -> Self {
        Self {
            cache: Some(FileCache::default()),
            ..self
        }
    }

    #[must_use]
    pub fn system_file(&self, domain: &Domain) -> PathBuf {
        let name = domain.file_name();
        self.system
            .iter()
            .map(|dir| dir.join(&name))
            .find(|file| file.is_file())
            .unwrap_or_else(|| match self.system.first() {
                Some(dir) => dir.join(&name),
                None => PathBuf::from(name),
            })
    }

    #[must_use]
    pub fn user_file(&self, domain: &Domain) -> PathBuf {
        self.user.join(domain.file_name())
    }

    pub(crate) fn read_user(&self, domain: &Domain) -> io::Result<Option<String>> {
        let path = self.user_file(domain);
        if let Some(stored) = self.cache.as_ref().and_then(|stored| stored.read(&path)) {
            return Ok(stored);
        }
        scribe::wait_for_writes(&path);
        atomic::read(&path)
    }

    pub(crate) fn write_user(
        &self,
        domain: &Domain,
        document: Option<String>,
    ) -> Option<io::Error> {
        let path = self.user_file(domain);
        match &self.cache {
            Some(stored) => {
                stored.write(path, document);
                None
            }
            None => scribe::scribe().write(path, document),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Default,
    System,
    User,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Default => "default",
            Self::System => "system",
            Self::User => "user",
        })
    }
}
