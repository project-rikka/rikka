use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ade_scope::{Scope, strings};

use crate::error::{Error, Result};
use crate::front::Front;
use crate::resources::Resources;

pub(crate) const SKILL: &str = "SKILL.md";

const NAMED: usize = 64;

const DESCRIBED: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    name: String,
    description: String,
    license: Option<String>,
    compatibility: Option<String>,
    metadata: BTreeMap<String, String>,
    allowed: Vec<String>,
    body: String,
    location: PathBuf,
    scope: Scope,
    resources: Resources,
}

impl Skill {
    pub(crate) fn read(folder: &Path, scope: Scope) -> Result<(Skill, Vec<String>)> {
        let location = folder.join(SKILL);
        let text = std::fs::read_to_string(&location).map_err(Error::Read)?;

        let (front, body) = Front::read(&text)?;
        let mut notes = Vec::new();

        let described = front.description.some().ok_or(Error::MissingDescription)?;
        let description = match strings::truncate(described, DESCRIBED) {
            None => described.to_owned(),
            Some(cut) => {
                notes.push(format!(
                    "description is longer than {DESCRIBED} characters and is truncated"
                ));
                cut
            }
        };

        let called = folder
            .file_name()
            .map_or_else(String::new, |folder| folder.to_string_lossy().into_owned());

        let name = match front.name.some() {
            None => {
                notes.push(format!("no name; using the folder name \"{called}\""));
                called
            }
            Some(name) => {
                if name != called {
                    notes.push(format!(
                        "name \"{name}\" does not match the folder name \"{called}\""
                    ));
                }
                if name.chars().count() > NAMED {
                    notes.push(format!("name is longer than {NAMED} characters"));
                }
                if !strings::is_valid_name(name) {
                    notes.push(format!(
                        "name \"{name}\" must use only lowercase letters, digits and single hyphens"
                    ));
                }
                name.to_owned()
            }
        };

        let skill = Skill {
            name,
            description,
            license: front.license.some().map(str::to_owned),
            compatibility: front.compatibility.some().map(str::to_owned),
            metadata: front.metadata.into_map(),
            allowed: front
                .allowed_tools
                .some()
                .into_iter()
                .flat_map(str::split_whitespace)
                .map(str::to_owned)
                .collect(),
            body: body.to_owned(),
            location,
            scope,
            resources: Resources::beside(folder),
        };
        Ok((skill, notes))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn compatibility(&self) -> Option<&str> {
        self.compatibility.as_deref()
    }

    #[must_use]
    pub fn license(&self) -> Option<&str> {
        self.license.as_deref()
    }

    #[must_use]
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    #[must_use]
    pub fn allowed(&self) -> &[String] {
        &self.allowed
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        &self.location
    }

    #[must_use]
    pub fn folder(&self) -> &Path {
        self.location.parent().unwrap_or(&self.location)
    }

    #[must_use]
    pub fn scope(&self) -> Scope {
        self.scope
    }

    #[must_use]
    pub fn resources(&self) -> &Resources {
        &self.resources
    }
}
