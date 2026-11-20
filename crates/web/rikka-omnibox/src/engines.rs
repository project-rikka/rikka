use crate::builtin::BUILTIN;
use crate::engine::SearchEngine;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEngines {
    engines: Vec<SearchEngine>,
    default: usize,
}

impl SearchEngines {
    #[must_use]
    pub fn builtin() -> SearchEngines {
        SearchEngines {
            engines: BUILTIN.to_vec(),
            default: 0,
        }
    }

    pub fn add(&mut self, engine: SearchEngine) -> Result<()> {
        if self.get(engine.id()).is_some() {
            return Err(Error::DuplicateId(engine.id().to_owned()));
        }
        if self.by_keyword(engine.keyword()).is_some() {
            return Err(Error::DuplicateKeyword(engine.keyword().to_owned()));
        }
        self.engines.push(engine);
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<SearchEngine> {
        let index = self.position(id)?;
        if self.engines[index].is_builtin() {
            return Err(Error::BuiltinEngine(id.to_owned()));
        }
        let removed = self.engines.remove(index);
        if self.default == index {
            self.default = 0;
        } else if self.default > index {
            self.default -= 1;
        }
        Ok(removed)
    }

    pub fn set_default(&mut self, id: &str) -> Result<()> {
        self.default = self.position(id)?;
        Ok(())
    }

    #[must_use]
    pub fn default_engine(&self) -> &SearchEngine {
        &self.engines[self.default]
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&SearchEngine> {
        self.engines.iter().find(|engine| engine.id() == id)
    }

    #[must_use]
    pub fn by_keyword(&self, keyword: &str) -> Option<&SearchEngine> {
        self.engines.iter().find(|engine| {
            engine
                .keyword()
                .chars()
                .eq(keyword.chars().flat_map(char::to_lowercase))
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &SearchEngine> {
        self.engines.iter()
    }

    fn position(&self, id: &str) -> Result<usize> {
        self.engines
            .iter()
            .position(|engine| engine.id() == id)
            .ok_or_else(|| Error::UnknownEngine(id.to_owned()))
    }
}

impl Default for SearchEngines {
    fn default() -> SearchEngines {
        SearchEngines::builtin()
    }
}
