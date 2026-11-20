use std::collections::BTreeMap;
use std::sync::OnceLock;

use rikka_prompt::{Facts, Prompt};

use crate::error::{Error, Result};
use crate::naming::Naming;

pub const ASK: &str = "ask";
pub const BASH: &str = "bash";
pub const COMPUTER: &str = "computer";
pub const DEFERRED: &str = "deferred";
pub const DESKTOP: &str = "desktop";
pub const EDIT: &str = "edit";
pub const JOB: &str = "job";
pub const READING: &str = "reading";
pub const TOOL_SEARCH: &str = "tool_search";
pub const TOOL_SEARCH_APPS: &str = "tool_search_apps";
pub const VIEW: &str = "view";
pub const WRITE: &str = "write";

const UNLISTED: [&str; 4] = [DEFERRED, DESKTOP, READING, TOOL_SEARCH_APPS];

const DOCUMENTS: [(&str, &str); 12] = [
    (ASK, include_str!("../prompts/ask.md")),
    (BASH, include_str!("../prompts/bash.md")),
    (COMPUTER, include_str!("../prompts/computer.md")),
    (DEFERRED, include_str!("../prompts/deferred.md")),
    (DESKTOP, include_str!("../prompts/desktop.md")),
    (EDIT, include_str!("../prompts/edit.md")),
    (JOB, include_str!("../prompts/job.md")),
    (READING, include_str!("../prompts/reading.md")),
    (TOOL_SEARCH, include_str!("../prompts/tool_search.md")),
    (
        TOOL_SEARCH_APPS,
        include_str!("../prompts/tool_search_apps.md"),
    ),
    (VIEW, include_str!("../prompts/view.md")),
    (WRITE, include_str!("../prompts/write.md")),
];

#[derive(Clone, Debug)]
pub struct Docs {
    prompts: BTreeMap<&'static str, Prompt>,
}

impl Docs {
    pub fn load() -> rikka_prompt::Result<Docs> {
        let mut prompts = BTreeMap::new();
        for (stem, text) in DOCUMENTS {
            prompts.insert(stem, Prompt::read(stem, text)?);
        }
        Ok(Docs { prompts })
    }

    pub fn get(&self, stem: &str) -> Result<&Prompt> {
        self.prompts
            .get(stem)
            .ok_or_else(|| Error::Unknown(stem.to_owned()))
    }

    #[must_use]
    pub fn facts(&self) -> Facts {
        let mut facts = Facts::new();
        for (stem, prompt) in self.named() {
            facts.set(format!("{stem}_tool"), prompt.name());
        }
        facts
    }

    #[must_use]
    pub fn naming(&self) -> Naming {
        Naming::from_names(
            self.named()
                .map(|(stem, prompt)| (stem, prompt.name().to_owned())),
        )
    }

    #[must_use]
    pub fn deferred(&self, machine: &str) -> Vec<&str> {
        self.named()
            .filter(|(stem, prompt)| *stem != TOOL_SEARCH && prompt.is_deferred())
            .map(|(stem, prompt)| match stem == COMPUTER {
                true => self.get(machine).unwrap_or(prompt).name(),
                false => prompt.name(),
            })
            .collect()
    }

    fn named(&self) -> impl Iterator<Item = (&'static str, &Prompt)> {
        self.prompts
            .iter()
            .filter(|(stem, _)| !UNLISTED.contains(*stem))
            .map(|(stem, prompt)| (*stem, prompt))
    }
}

pub fn facts() -> rikka_prompt::Result<Facts> {
    Ok(Docs::load()?.facts())
}

#[must_use]
pub fn naming() -> &'static Naming {
    static NAMING: OnceLock<Naming> = OnceLock::new();
    NAMING.get_or_init(|| Docs::load().map(|docs| docs.naming()).unwrap_or_default())
}
