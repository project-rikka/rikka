use rikka_nix_edit::Document;
use rikka_nix_protocol::{Action, Request};
use rikka_nixos_layout::{Module, Modules};

use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    saved: Modules,
    edited: Modules,
}

impl Draft {
    #[must_use]
    pub fn new(saved: Modules) -> Self {
        Self {
            edited: saved.clone(),
            saved,
        }
    }

    pub fn system() -> Result<Self> {
        Ok(Self::new(Modules::system()?))
    }

    #[must_use]
    pub fn saved(&self, module: Module) -> &Document {
        self.saved.get(module)
    }

    #[must_use]
    pub fn edited(&self, module: Module) -> &Document {
        self.edited.get(module)
    }

    pub fn edited_mut(&mut self, module: Module) -> &mut Document {
        self.edited.get_mut(module)
    }

    #[must_use]
    pub fn is_changed(&self) -> bool {
        self.edited != self.saved
    }

    pub fn discard(&mut self) {
        self.edited.clone_from(&self.saved);
    }

    pub fn rebase(&mut self, current: Modules) -> Vec<Module> {
        let mut overridden = Vec::new();
        for module in Module::ALL {
            let theirs = current.get(module);
            let untouched_here = self.edited.get(module) == self.saved.get(module);
            let untouched_there = theirs == self.saved.get(module);
            if untouched_there || self.edited.get(module) == theirs {
                continue;
            }
            if !untouched_here {
                overridden.push(module);
            }
            self.edited.get_mut(module).clone_from(theirs);
        }
        self.saved = current;
        overridden
    }

    #[must_use]
    pub fn request(&self, action: Action) -> Request {
        Request::apply(&self.saved, &self.edited, action)
    }
}
