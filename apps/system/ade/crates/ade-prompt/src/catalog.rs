use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use rikka_prompt::{Facts, Prompt};
use rikka_tools::Naming;

use crate::error::{Error, Result};

pub const AGENT: &str = "agent";
pub const SKILL: &str = "skill";

const SYSTEM: &str = include_str!("../../../prompts/system.md");
const ENVIRONMENT: &str = include_str!("../../../prompts/environment.md");
const GUIDES: &str = include_str!("../../../prompts/guides.md");
const MEMORY: &str = include_str!("../../../prompts/memory.md");
const SKILLS: &str = include_str!("../../../prompts/skills.md");
const COMPACT: &str = include_str!("../../../prompts/compact.md");
const INTERJECTED: &str = include_str!("../../../prompts/interjected.md");
const SUBAGENT: &str = include_str!("../../../prompts/subagent.md");

const TOOLS: [(&str, &str); 2] = [
    (AGENT, include_str!("../../../prompts/tools/agent.md")),
    (SKILL, include_str!("../../../prompts/tools/skill.md")),
];

#[derive(Clone, Debug)]
pub struct Prompts {
    system: Prompt,
    environment: Prompt,
    guides: Prompt,
    memory: Prompt,
    skills: Prompt,
    compact: Prompt,
    interjected: Prompt,
    subagent: Prompt,
    tools: BTreeMap<&'static str, Prompt>,
}

impl Prompts {
    pub fn load() -> Result<Prompts> {
        let mut tools = BTreeMap::new();
        for (stem, text) in TOOLS {
            tools.insert(stem, Prompt::read(stem, text)?);
        }

        Ok(Prompts {
            system: Prompt::read("system", SYSTEM)?,
            environment: Prompt::read("environment", ENVIRONMENT)?,
            guides: Prompt::read("guides", GUIDES)?,
            memory: Prompt::read("memory", MEMORY)?,
            skills: Prompt::read("skills", SKILLS)?,
            compact: Prompt::read("compact", COMPACT)?,
            interjected: Prompt::read("interjected", INTERJECTED)?,
            subagent: Prompt::read("subagent", SUBAGENT)?,
            tools,
        })
    }

    pub fn facts(&self, project: &Path, language: &str) -> Result<Facts> {
        let mut facts = rikka_tools::facts_for(language)?;
        facts.set("project", project.display().to_string());
        for (stem, prompt) in &self.tools {
            facts.set(format!("{stem}_tool"), prompt.name());
        }
        Ok(facts)
    }

    #[must_use]
    pub fn system(&self) -> &Prompt {
        &self.system
    }

    #[must_use]
    pub fn guides(&self) -> &Prompt {
        &self.guides
    }

    #[must_use]
    pub fn memory(&self) -> &Prompt {
        &self.memory
    }

    #[must_use]
    pub fn skills(&self) -> &Prompt {
        &self.skills
    }

    #[must_use]
    pub fn environment(&self) -> &Prompt {
        &self.environment
    }

    #[must_use]
    pub fn compact(&self) -> &Prompt {
        &self.compact
    }

    #[must_use]
    pub fn interjected(&self) -> &Prompt {
        &self.interjected
    }

    #[must_use]
    pub fn subagent(&self) -> &Prompt {
        &self.subagent
    }

    #[must_use]
    pub fn naming(&self) -> Naming {
        let mut naming = rikka_tools::naming().clone();
        for (stem, prompt) in &self.tools {
            naming = naming.with(stem, prompt.name());
        }
        naming
    }

    pub fn tool(&self, stem: &str) -> Result<&Prompt> {
        self.tools
            .get(stem)
            .ok_or_else(|| Error::Unknown(stem.to_owned()))
    }
}

#[must_use]
pub fn naming() -> &'static Naming {
    static NAMING: OnceLock<Naming> = OnceLock::new();
    NAMING.get_or_init(|| {
        Prompts::load()
            .map(|prompts| prompts.naming())
            .unwrap_or_default()
    })
}
