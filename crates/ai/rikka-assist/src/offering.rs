use std::collections::BTreeMap;

use crate::error::{Error, Result};

const LONGEST: usize = 24;

#[derive(Clone, Debug)]
pub struct Offering {
    kit: String,
    title: String,
    about: String,
    instructions: String,
    gists: BTreeMap<String, Vec<String>>,
}

#[must_use]
pub fn is_kit(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && name.len() <= LONGEST
        && name.chars().all(|letter| letter.is_ascii_alphanumeric())
}

impl Offering {
    pub fn new(kit: impl Into<String>) -> Result<Offering> {
        let kit = kit.into();
        if !is_kit(&kit) {
            return Err(Error::Unnamable(kit));
        }
        Ok(Offering {
            title: kit.clone(),
            kit,
            about: String::new(),
            instructions: String::new(),
            gists: BTreeMap::new(),
        })
    }

    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Offering {
        self.title = title.into();
        self
    }

    #[must_use]
    pub fn with_description(mut self, about: impl Into<String>) -> Offering {
        self.about = about.into();
        self
    }

    #[must_use]
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Offering {
        self.instructions = instructions.into();
        self
    }

    #[must_use]
    pub fn gist<S: Into<String>>(
        mut self,
        tool: impl Into<String>,
        paths: impl IntoIterator<Item = S>,
    ) -> Offering {
        self.gists
            .insert(tool.into(), paths.into_iter().map(Into::into).collect());
        self
    }

    #[must_use]
    pub fn kit(&self) -> &str {
        &self.kit
    }

    pub(crate) fn title(&self) -> &str {
        &self.title
    }

    pub(crate) fn about(&self) -> &str {
        &self.about
    }

    pub(crate) fn instructions(&self) -> &str {
        &self.instructions
    }

    pub(crate) fn gist_of(&self, tool: &str) -> Vec<String> {
        self.gists.get(tool).cloned().unwrap_or_default()
    }
}
