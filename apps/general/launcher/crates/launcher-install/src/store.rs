use std::path::{Path, PathBuf};

const VERSIONS: &str = "versions";
const LIBRARIES: &str = "libraries";
const ASSETS: &str = "assets";
const RUNTIMES: &str = "runtimes";
const NATIVES: &str = "natives";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Store {
        Store { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn manifest(&self) -> PathBuf {
        self.root.join(VERSIONS).join("version_manifest_v2.json")
    }

    #[must_use]
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.root.join(VERSIONS).join(id).join(format!("{id}.json"))
    }

    #[must_use]
    pub fn client_jar(&self, id: &str) -> PathBuf {
        self.root.join(VERSIONS).join(id).join(format!("{id}.jar"))
    }

    #[must_use]
    pub fn libraries(&self) -> PathBuf {
        self.root.join(LIBRARIES)
    }

    #[must_use]
    pub fn assets(&self) -> PathBuf {
        self.root.join(ASSETS)
    }

    #[must_use]
    pub fn asset_index(&self, id: &str) -> PathBuf {
        self.assets().join("indexes").join(format!("{id}.json"))
    }

    #[must_use]
    pub fn asset_objects(&self) -> PathBuf {
        self.assets().join("objects")
    }

    #[must_use]
    pub fn natives(&self, id: &str) -> PathBuf {
        self.root.join(NATIVES).join(id)
    }

    #[must_use]
    pub fn runtime(&self, component: &str) -> PathBuf {
        self.root.join(RUNTIMES).join(component)
    }

    #[must_use]
    pub fn runtime_index(&self) -> PathBuf {
        self.root.join(RUNTIMES).join("all.json")
    }

    #[must_use]
    pub fn runtime_manifest(&self, component: &str) -> PathBuf {
        self.root.join(RUNTIMES).join(format!("{component}.json"))
    }
}
