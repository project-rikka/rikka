use std::io;
use std::path::{Path, PathBuf};

use rikka_dirs::Paths;

use crate::atomic;
use crate::domain::Domain;
use crate::error::Result;
use crate::file_cache::FileCache;
use crate::scribe;
use crate::source::Scope;

#[cfg(unix)]
const DECLARED: &str = "/etc/rikka/settings";

#[cfg(unix)]
const MACHINE: &str = "machine";

#[cfg(unix)]
const USERS: &str = "users";

const LOCKED: &str = "locked";

#[derive(Clone, Debug)]
pub struct Layers {
    system: Vec<PathBuf>,
    declared: Vec<Declared>,
    user: PathBuf,
    cache: Option<FileCache>,
}

#[derive(Clone, Debug)]
struct Declared {
    scope: Scope,
    folder: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct DeclaredFiles {
    pub(crate) scope: Scope,
    pub(crate) values: PathBuf,
    pub(crate) locked: PathBuf,
}

impl Layers {
    pub fn resolve() -> Result<Self> {
        let paths = Paths::resolve()?;
        rikka_dirs::ensure(paths.config_dir())?;
        let layers = Self::new_with(paths.system_config_dirs().to_vec(), paths.config_dir());
        Ok(declared_on_this_machine(layers))
    }

    pub fn new(system: impl Into<PathBuf>, user: impl Into<PathBuf>) -> Self {
        Self::new_with(vec![system.into()], user)
    }

    fn new_with(system: Vec<PathBuf>, user: impl Into<PathBuf>) -> Self {
        Self {
            system,
            declared: Vec::new(),
            user: user.into(),
            cache: None,
        }
    }

    #[must_use]
    pub fn declared(mut self, scope: Scope, folder: impl Into<PathBuf>) -> Self {
        self.declared.retain(|declared| declared.scope != scope);
        self.declared.push(Declared {
            scope,
            folder: folder.into(),
        });
        self.declared.sort_by_key(|declared| declared.scope);
        self
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

    #[must_use]
    pub fn declared_folder(&self, scope: Scope) -> Option<&Path> {
        self.declared
            .iter()
            .find(|declared| declared.scope == scope)
            .map(|declared| declared.folder.as_path())
    }

    pub(crate) fn declared_files(&self, domain: &Domain) -> impl Iterator<Item = DeclaredFiles> {
        let name = domain.file_name();
        self.declared.iter().map(move |declared| DeclaredFiles {
            scope: declared.scope,
            values: declared.folder.join(&name),
            locked: declared.folder.join(LOCKED).join(&name),
        })
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

#[cfg(unix)]
fn declared_on_this_machine(layers: Layers) -> Layers {
    let root = Path::new(DECLARED);
    let layers = layers.declared(Scope::Machine, root.join(MACHINE));
    match rikka_dirs::passwd::current() {
        Some(account) => layers.declared(Scope::User, root.join(USERS).join(account.name)),
        None => {
            tracing::warn!("failed to look up this account, so no settings are declared for it");
            layers
        }
    }
}

#[cfg(windows)]
fn declared_on_this_machine(layers: Layers) -> Layers {
    layers
}
