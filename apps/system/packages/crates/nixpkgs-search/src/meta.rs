use nixpkgs_catalog::{License, Maintainer, Package, Team};
use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Deserialize)]
pub(crate) struct Meta {
    attribute: String,
    set: String,
    name: String,
    version: String,
    description: String,
    about: String,
    main_program: String,
    outputs: Vec<String>,
    platforms: Vec<String>,
    unfree: bool,
    licences: Vec<License>,
    maintainers: Vec<Maintainer>,
    teams: Vec<Team>,
    homepages: Vec<String>,
    position: String,
}

impl From<Meta> for Package {
    fn from(meta: Meta) -> Self {
        Self {
            attribute: meta.attribute,
            set: named(meta.set),
            name: meta.name,
            version: meta.version,
            description: meta.description,
            about: meta.about,
            main_program: named(meta.main_program),
            outputs: meta.outputs,
            platforms: meta.platforms,
            unfree: meta.unfree,
            licenses: meta.licences,
            maintainers: meta.maintainers,
            teams: meta.teams,
            homepages: meta.homepages,
            position: named(meta.position),
        }
    }
}

fn named(value: String) -> Option<String> {
    Some(value).filter(|value| !value.is_empty())
}

pub(crate) fn read(answered: &[u8]) -> Result<Vec<Package>> {
    let read: Vec<Meta> = serde_json::from_slice(answered)?;
    Ok(read.into_iter().map(Into::into).collect())
}
