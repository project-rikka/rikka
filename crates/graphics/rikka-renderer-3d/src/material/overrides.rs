use crate::material::value::Value;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    All,
    Template(String),
}

impl Scope {
    pub(crate) fn covers(&self, template: &str) -> bool {
        match self {
            Self::All => true,
            Self::Template(name) => name == template,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Override {
    pub scope: Scope,
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    entries: Vec<Override>,
}

impl Overrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, value: impl Into<Value>) -> &mut Self {
        self.put(Scope::All, name, value.into())
    }

    pub fn set_for(&mut self, template: &str, name: &str, value: impl Into<Value>) -> &mut Self {
        self.put(Scope::Template(template.to_owned()), name, value.into())
    }

    #[must_use]
    pub fn with(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.set(name, value);
        self
    }

    #[must_use]
    pub fn with_for(mut self, template: &str, name: &str, value: impl Into<Value>) -> Self {
        self.set_for(template, name, value);
        self
    }

    fn put(&mut self, scope: Scope, name: &str, value: Value) -> &mut Self {
        match self
            .entries
            .iter_mut()
            .find(|entry| entry.scope == scope && entry.name == name)
        {
            Some(entry) => entry.value = value,
            None => self.entries.push(Override {
                scope,
                name: name.to_owned(),
                value,
            }),
        }
        self
    }

    pub fn remove(&mut self, name: &str) -> &mut Self {
        self.entries.retain(|entry| entry.name != name);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.name == name)
            .map(|entry| &entry.value)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Override> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn applying<'a>(
        &'a self,
        template: &'a str,
    ) -> impl Iterator<Item = &'a Override> + 'a {
        self.entries
            .iter()
            .filter(move |entry| entry.scope.covers(template))
    }
}
