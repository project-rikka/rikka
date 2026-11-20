use std::path::Path;
use std::sync::Arc;

use ade_guide::Nearby;
use ade_memory::DESCRIBED;
use ade_prompt::{AGENT, Facts, Prompts, SKILL};
use rikka_agent::CompactionPrompts;
use rikka_computer_use::ComputerUse;
use rikka_llm_accounts::Llm;
use rikka_mcp::Hub;
use rikka_thread::Assistant;
use rikka_tools::Toolbox;

use crate::project::catalogs::Catalogs;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AssemblyError {
    #[error(transparent)]
    Prompts(#[from] ade_prompt::Error),

    #[error(transparent)]
    Document(#[from] rikka_prompt::Error),

    #[error(transparent)]
    Skills(#[from] ade_skill::Error),

    #[error(transparent)]
    Tools(#[from] rikka_tools::Error),

    #[error(transparent)]
    Thread(#[from] rikka_thread::Error),
}

#[derive(Clone, Debug)]
pub(crate) struct Assembly {
    pub(crate) hub: Hub,
    pub(crate) models: String,
    pub(crate) language: String,
}

impl Assembly {
    pub(crate) fn new(hub: Hub, llm: &Llm) -> Assembly {
        Assembly {
            hub,
            models: llm.roster().summary(),
            language: llm.reply_language().to_owned(),
        }
    }
}

pub(crate) fn assemble(
    root: &Path,
    assembly: &Assembly,
    catalogs: &Catalogs,
) -> Result<Assistant, AssemblyError> {
    let prompts = Prompts::load()?;
    let mut facts = prompts.facts(root, &assembly.language)?;
    facts.set("models", assembly.models.clone());
    let standing = standing(&prompts, &facts, catalogs, &assembly.hub)?;
    let skills = &catalogs.skills;

    let mut setup = assembly.hub.equip(
        Toolbox::builder(root)
            .machine(ComputerUse::machine())
            .web()
            .path_instructions(Arc::new(Nearby::new(root))),
    );

    if !skills.is_empty() {
        setup = setup.tools([ade_skill::declare(skills, prompts.tool(SKILL)?, &facts)?]);
    }

    let mut briefing = standing.clone();
    briefing.push(prompts.subagent().body(&facts)?);

    let mut making = Assistant::builder(setup.build()?)
        .interjection_lead(prompts.interjected().body(&facts)?)
        .compaction_prompts(CompactionPrompts::new(prompts.compact().body(&facts)?))
        .subagents(briefing, prompts.tool(AGENT)?, &facts);

    for instruction in standing {
        making = making.instruction(instruction);
    }
    Ok(making.build()?)
}

fn standing(
    prompts: &Prompts,
    facts: &Facts,
    catalogs: &Catalogs,
    hub: &Hub,
) -> Result<Vec<String>, AssemblyError> {
    let mut standing = vec![prompts.system().body(facts)?];

    if !catalogs.guides.is_empty() {
        let mut facts = facts.clone();
        facts.set("guides", catalogs.guides.render());
        standing.push(prompts.guides().body(&facts)?);
    }

    let memory = catalogs.memories.render();
    if !memory.is_empty() {
        let mut facts = facts.clone();
        facts.set("memory", memory);
        facts.set("described", DESCRIBED.to_string());
        standing.push(prompts.memory().body(&facts)?);
    }

    if !catalogs.skills.is_empty() {
        let mut facts = facts.clone();
        facts.set("skills", catalogs.skills.catalogue());
        standing.push(prompts.skills().body(&facts)?);
    }

    standing.push(hub.instruction());
    standing.push(prompts.environment().body(facts)?);
    Ok(standing)
}
