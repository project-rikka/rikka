use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Naming {
    stems: HashMap<String, &'static str>,
}

impl Naming {
    pub(crate) fn from_names(named: impl Iterator<Item = (&'static str, String)>) -> Naming {
        Naming {
            stems: named.map(|(stem, name)| (name, stem)).collect(),
        }
    }

    #[must_use]
    pub fn with(mut self, stem: &'static str, name: impl Into<String>) -> Naming {
        self.stems.insert(name.into(), stem);
        self
    }

    #[must_use]
    pub fn stem(&self, name: &str) -> Option<&'static str> {
        self.stems.get(name).copied()
    }
}
