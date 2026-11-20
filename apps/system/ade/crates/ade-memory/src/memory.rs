use std::path::{Component, Path, PathBuf};

use ade_scope::strings::{self, collapse_whitespace};

use crate::error::{Error, Result};
use crate::front::Front;

pub const DESCRIBED: usize = 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    name: String,
    title: String,
    description: String,
    about: Option<String>,
    tags: Vec<String>,
    path: PathBuf,
}

impl Memory {
    pub(crate) fn read(path: &Path, root: &Path) -> Result<(Memory, Vec<String>)> {
        let text = std::fs::read_to_string(path).map_err(Error::Read)?;

        let (front, _) = Front::read(&text)?;
        let mut notes = Vec::new();

        let described = front.description.some().ok_or(Error::MissingDescription)?;
        let description = truncate(&collapse_whitespace(described), &mut notes);

        let name = path
            .file_stem()
            .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned());
        if !strings::is_valid_name(&name) {
            notes.push(format!(
                "file name \"{name}\" must use only lowercase letters, digits and single hyphens"
            ));
        }

        let title = match front.title.some() {
            Some(title) => collapse_whitespace(title),
            None => {
                notes.push(format!("no title; using the file name \"{name}\""));
                name.clone()
            }
        };

        let about = front
            .about
            .some()
            .and_then(|about| within(about, &mut notes));
        if let Some(about) = &about
            && !root.join(about).exists()
        {
            notes.push(format!("about path \"{about}\" does not exist"));
        }

        let memory = Memory {
            name,
            title,
            description,
            about,
            tags: front.tags.into_vec(),
            path: path.to_path_buf(),
        };
        Ok((memory, notes))
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn about(&self) -> Option<&str> {
        self.about.as_deref()
    }

    #[must_use]
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn truncate(described: &str, notes: &mut Vec<String>) -> String {
    let Some(cut) = strings::truncate(described, DESCRIBED) else {
        return described.to_owned();
    };

    notes.push(format!(
        "description is longer than {DESCRIBED} characters and is truncated"
    ));
    cut
}

fn within(about: &str, notes: &mut Vec<String>) -> Option<String> {
    let mut inside = Vec::new();

    for part in Path::new(about.trim()).components() {
        match part {
            Component::Normal(name) => inside.push(name.to_string_lossy().into_owned()),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            Component::ParentDir => {
                notes.push(format!(
                    "about path \"{about}\" is outside the project and is ignored"
                ));
                return None;
            }
        }
    }

    (!inside.is_empty()).then(|| inside.join("/"))
}
