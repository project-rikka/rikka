use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use rikka_nix_edit::Document;

use crate::error::{Error, Result};
use crate::location::Location;
use crate::module::Module;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modules {
    documents: BTreeMap<Module, Document>,
}

impl Modules {
    #[must_use]
    pub fn blank() -> Self {
        Self {
            documents: Module::EVERY
                .into_iter()
                .map(|module| (module, module.blank()))
                .collect(),
        }
    }

    pub fn system() -> Result<Self> {
        Self::read(Location::system()?.modules())
    }

    pub fn read(directory: impl AsRef<Path>) -> Result<Self> {
        let directory = directory.as_ref();
        let mut documents = BTreeMap::from([(Module::Root, Module::Root.blank())]);
        for module in Module::ALL {
            let path = directory.join(module.file_name());
            let document = match std::fs::read_to_string(&path) {
                Ok(source) => {
                    Document::parse(source).map_err(|source| Error::Invalid { path, source })?
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => module.blank(),
                Err(source) => return Err(Error::Unreadable { path, source }),
            };
            documents.insert(module, document);
        }
        Ok(Self { documents })
    }

    #[must_use]
    pub fn get(&self, module: Module) -> &Document {
        &self.documents[&module]
    }

    pub fn get_mut(&mut self, module: Module) -> &mut Document {
        self.documents
            .get_mut(&module)
            .expect("all modules are loaded at construction")
    }

    pub fn iter(&self) -> impl Iterator<Item = (Module, &Document)> {
        self.documents
            .iter()
            .map(|(module, document)| (*module, document))
    }
}
