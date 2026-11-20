use std::path::{Path, PathBuf};

use ade_scope::{Scope, strings};

use crate::error::{Error, Result};
use crate::filling;
use crate::front::Front;
use crate::naming;

const DESCRIBED: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    name: String,
    description: String,
    hint: Option<String>,
    body: String,
    path: PathBuf,
    scope: Scope,
}

impl Command {
    pub(crate) fn read(path: &Path, scope: Scope) -> Result<Command> {
        let text = std::fs::read_to_string(path).map_err(Error::Read)?;

        let (front, body) = Front::read(&text)?;
        if body.is_empty() {
            return Err(Error::Empty);
        }

        let name = path
            .file_stem()
            .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
        if !naming::is_valid_name(&name) {
            return Err(Error::InvalidName(name));
        }

        Ok(Command {
            description: described(front.description.some(), body),
            hint: front.argument_hint.some().map(str::to_owned),
            body: body.to_owned(),
            path: path.to_path_buf(),
            scope,
            name,
        })
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
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    #[must_use]
    pub fn body(&self) -> &str {
        &self.body
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn scope(&self) -> Scope {
        self.scope
    }

    #[must_use]
    pub fn filled(&self, arguments: &str) -> String {
        filling::fill(&self.body, arguments)
    }
}

fn described(explicit: Option<&str>, body: &str) -> String {
    let described = strings::collapse_whitespace(
        explicit.unwrap_or_else(|| body.lines().next().unwrap_or_default()),
    );
    strings::truncate(&described, DESCRIBED).unwrap_or(described)
}
