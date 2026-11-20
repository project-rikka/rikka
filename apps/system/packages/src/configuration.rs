use std::collections::BTreeSet;
use std::path::Path;

use rikka_nix::{
    Action, CATALOG, Distribution, Document, Draft, Module, Modules, Packages, Request,
};

use crate::apps::Apps;
use crate::integration::Writes;

const SYSTEM_PACKAGES: &str = "environment.systemPackages";

const ALLOW_UNFREE: &str = "nixpkgs.config.allowUnfree";

#[derive(Debug)]
pub(crate) struct Declared {
    pub(crate) packages: Vec<String>,
    pub(crate) integration: Document,
    pub(crate) unfree: bool,
    pub(crate) distribution: Option<Distribution>,
    pub(crate) apps: BTreeSet<String>,
    pub(crate) modules: Modules,
}

impl Declared {
    pub(crate) fn read() -> rikka_nix::Result<Self> {
        let modules = Modules::system()?;
        let packages = modules
            .get(Module::Packages)
            .get::<Packages>(SYSTEM_PACKAGES)?
            .unwrap_or_default();
        let distribution = match Path::new(CATALOG).exists() {
            true => Some(Distribution::system()?),
            false => None,
        };
        let apps = match &distribution {
            Some(distribution) => distribution.chosen(modules.get(Module::Apps))?,
            None => BTreeSet::new(),
        };
        Ok(Self {
            packages: packages.names().collect(),
            integration: modules.get(Module::Integration).clone(),
            unfree: modules.get(Module::Nix).get::<bool>(ALLOW_UNFREE)? == Some(true),
            distribution,
            apps,
            modules,
        })
    }
}

#[derive(Debug)]
pub(crate) struct Changes {
    packages: Vec<String>,
    writes: Writes,
    apps: Apps,
}

impl Changes {
    pub(crate) fn new(packages: Vec<String>, writes: Writes, apps: Apps) -> Self {
        Self {
            packages,
            writes,
            apps,
        }
    }

    pub(crate) fn request(&self, declared: Modules) -> rikka_nix::Result<Request> {
        let mut draft = Draft::new(declared);
        draft
            .edited_mut(Module::Packages)
            .set(SYSTEM_PACKAGES, Packages::new(&self.packages))?;
        self.writes
            .apply_to(draft.edited_mut(Module::Integration))?;
        self.apps.write(draft.edited_mut(Module::Apps))?;
        if self.writes.unfree {
            draft.edited_mut(Module::Nix).set(ALLOW_UNFREE, true)?;
        }
        Ok(draft.request(Action::Switch))
    }
}
