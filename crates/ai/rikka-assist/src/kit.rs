use std::sync::OnceLock;

use rikka_prompt::{Facts, Prompt};

use crate::error::{Error, Result};
use crate::offering::Offering;
use crate::protocol::exposed;

type FactsOf = fn(&mut Facts) -> rikka_prompt::Result<()>;

#[derive(Clone, Copy, Debug)]
pub struct KitTool {
    pub stem: &'static str,
    pub document: &'static str,
    pub gist: &'static [&'static str],
}

pub struct Kit {
    name: &'static str,
    about: &'static str,
    system: &'static str,
    tools: &'static [KitTool],
    facts: FactsOf,
    prepared: OnceLock<std::result::Result<Prepared, String>>,
}

#[derive(Debug)]
pub struct Prepared {
    tools: Vec<Prompt>,
    facts: Facts,
    instructions: String,
}

impl Kit {
    #[must_use]
    pub const fn new(
        name: &'static str,
        about: &'static str,
        system: &'static str,
        tools: &'static [KitTool],
        facts: FactsOf,
    ) -> Kit {
        Kit {
            name,
            about,
            system,
            tools,
            facts,
            prepared: OnceLock::new(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn prepared(&self) -> Result<&Prepared> {
        self.prepared
            .get_or_init(|| self.prepare().map_err(|error| error.to_string()))
            .as_ref()
            .map_err(|error| Error::Unprepared(error.clone()))
    }

    pub fn offering(&self, title: impl Into<String>) -> Result<Offering> {
        let prepared = self.prepared()?;
        let offering = Offering::new(self.name)?
            .with_title(title)
            .with_description(self.about)
            .with_instructions(prepared.instructions.as_str());
        Ok(self
            .tools
            .iter()
            .zip(&prepared.tools)
            .fold(offering, |offering, (tool, prompt)| {
                offering.gist(prompt.name(), tool.gist.iter().copied())
            }))
    }

    fn prepare(&self) -> rikka_prompt::Result<Prepared> {
        let tools = self
            .tools
            .iter()
            .map(|tool| Prompt::read(tool.stem, tool.document))
            .collect::<rikka_prompt::Result<Vec<Prompt>>>()?;
        let mut facts = rikka_tool_docs::facts()?;
        for (tool, prompt) in self.tools.iter().zip(&tools) {
            facts.set(
                format!("{}_tool", tool.stem),
                exposed(self.name, prompt.name()),
            );
        }
        (self.facts)(&mut facts)?;
        let instructions = Prompt::read("system", self.system)?.body(&facts)?;
        Ok(Prepared {
            tools,
            facts,
            instructions,
        })
    }
}

impl std::fmt::Debug for Kit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kit")
            .field("name", &self.name)
            .field("tools", &self.tools.len())
            .finish_non_exhaustive()
    }
}

impl Prepared {
    #[must_use]
    pub fn tool(&self, index: usize) -> &Prompt {
        &self.tools[index]
    }

    #[must_use]
    pub fn facts(&self) -> &Facts {
        &self.facts
    }
}
