use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rikka_nix_edit::Document;
use serde::Deserialize;

use crate::error::{Error, Result};

pub const CATALOG: &str = "/etc/rikka/catalog.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Offer {
    Required,
    Selected,
    Offered,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Component {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    pub summary: String,
    pub offer: Offer,
    pub version: String,
    pub installed: bool,
}

impl Component {
    #[must_use]
    pub fn is_optional(&self) -> bool {
        self.offer != Offer::Required
    }
}

#[derive(Debug, Deserialize)]
struct Listing {
    components: BTreeMap<String, Component>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Distribution {
    components: Vec<Component>,
}

impl Distribution {
    pub fn system() -> Result<Distribution> {
        let path = Path::new(CATALOG);
        let json = std::fs::read_to_string(path).map_err(|error| Error::Unreadable {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;
        Self::parse(&json)
    }

    pub fn parse(json: &str) -> Result<Distribution> {
        let listing: Listing =
            serde_json::from_str(json).map_err(|error| Error::Catalog(error.to_string()))?;
        let mut components: Vec<Component> = listing
            .components
            .into_iter()
            .map(|(id, component)| Component { id, ..component })
            .collect();
        components.sort_by_key(|component| (component.offer, component.name.to_lowercase()));
        Ok(Distribution { components })
    }

    #[must_use]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|component| component.id == id)
    }

    pub fn chosen(&self, apps: &Document) -> Result<BTreeSet<String>> {
        let mut chosen = BTreeSet::new();
        for component in self.optional() {
            if apps.get::<bool>(enable_path(&component.id))? == Some(true) {
                chosen.insert(component.id.clone());
            }
        }
        Ok(chosen)
    }

    pub fn choose(&self, apps: &mut Document, chosen: &BTreeSet<String>) -> Result<()> {
        for component in self.optional() {
            let path = enable_path(&component.id);
            match chosen.contains(&component.id) {
                true => apps.set(path, true)?,
                false => apps.remove(path)?,
            }
        }
        Ok(())
    }

    fn optional(&self) -> impl Iterator<Item = &Component> {
        self.components
            .iter()
            .filter(|component| component.is_optional())
    }
}

fn enable_path(id: &str) -> [&str; 4] {
    ["rikka", "apps", id, "enable"]
}
