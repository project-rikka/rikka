use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{Error, Result};

pub const PACKAGES_FLAKE: &str = "github:project-rikka/packages";

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
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    components: Vec<Component>,
}

impl Catalog {
    pub fn parse(json: &str) -> Result<Catalog> {
        let listed: BTreeMap<String, Component> =
            serde_json::from_str(json).map_err(|error| Error::Catalog(error.to_string()))?;
        let mut components: Vec<Component> = listed
            .into_iter()
            .map(|(id, component)| Component { id, ..component })
            .collect();
        components.sort_by_key(|component| (component.offer, component.name.to_lowercase()));
        Ok(Catalog { components })
    }

    #[must_use]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    pub fn optional(&self) -> impl Iterator<Item = &Component> {
        self.components
            .iter()
            .filter(|component| component.offer != Offer::Required)
    }

    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|component| component.id == id)
    }
}
