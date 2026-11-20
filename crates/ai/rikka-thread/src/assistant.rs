use std::sync::Arc;

use rikka_agent::{
    Agent, Budget, Compaction, CompactionPrompts, Function, Interjections, Notices, Observer,
};
use rikka_file_edit::History;
use rikka_llm::ModelInfo;
use rikka_prompt::{Facts, Prompt};
use rikka_tools::{Jobs, Machine, Questions, ToolKitEntry, Toolbox};

use crate::delegate;
use crate::error::{Result, setup_error};
use crate::fitting::{Fitting, applied};
use crate::run_context::RunContext;
use crate::store::Crew;
use crate::subagents::Subagents;
use crate::thread::Thread;

const REVISED: &str = include_str!("../prompts/revised.md");

fn budget() -> Budget {
    Budget::new().turns(u32::MAX)
}

#[must_use]
pub fn is_near_context_limit(filled: u64, model: &ModelInfo) -> bool {
    Compaction::is_near_limit(filled, model.context(), u64::from(model.output()))
}

#[derive(Debug)]
struct SubagentSetup {
    briefing: Vec<String>,
    prompt: Prompt,
    facts: Facts,
}

#[derive(Debug)]
struct Setup {
    instructions: Vec<String>,
    revision_heading: String,
    interjection_lead: Option<String>,
    folding: Option<CompactionPrompts>,
    subagent_setup: Option<SubagentSetup>,
}

#[derive(Debug)]
pub struct Assistant {
    setup: Arc<Setup>,
    toolbox: Arc<Toolbox>,
    questions: Questions,
    subagents: Option<Subagents>,
    delegating: Option<Function>,
}

#[derive(Debug)]
pub struct AssistantBuilder {
    toolbox: Toolbox,
    setup: Setup,
}

impl Assistant {
    #[must_use]
    pub fn builder(toolbox: Toolbox) -> AssistantBuilder {
        AssistantBuilder {
            toolbox,
            setup: Setup {
                instructions: Vec::new(),
                revision_heading: REVISED.trim().to_owned(),
                interjection_lead: None,
                folding: None,
                subagent_setup: None,
            },
        }
    }

    pub fn fresh(&self) -> Result<Assistant> {
        let toolbox = Arc::new(self.toolbox.fresh().map_err(setup_error)?);
        Assistant::new(Arc::clone(&self.setup), toolbox)
    }

    #[must_use]
    pub fn questions(&self) -> &Questions {
        &self.questions
    }

    #[must_use]
    pub fn machine(&self) -> Option<&dyn Machine> {
        self.toolbox.machine()
    }

    #[must_use]
    pub fn jobs(&self) -> &Jobs {
        self.toolbox.jobs()
    }

    #[must_use]
    pub fn notices(&self) -> &Notices {
        self.toolbox.jobs().notices()
    }

    #[must_use]
    pub fn toolkit(&self, name: &str) -> Option<ToolKitEntry> {
        self.toolbox.toolkit(name)
    }

    #[must_use]
    pub fn open_toolkit(&self, name: &str) -> Option<ToolKitEntry> {
        self.toolbox.open_toolkit(name)
    }

    #[must_use]
    pub fn subagents(&self) -> Option<&Subagents> {
        self.subagents.as_ref()
    }

    pub fn keep_files_in(&self, history: History, thread: &Thread) {
        if let Some(last) = thread.last_asked() {
            history.set_checkpoint(last);
        }
        self.toolbox.keep_files_in(history);
    }

    pub fn keep_agents_in(&self, crew: Crew) {
        if let Some(subagents) = &self.subagents {
            subagents.keep_in(crew);
        }
    }

    pub(crate) fn history(&self) -> Option<History> {
        self.toolbox.history()
    }

    pub(crate) fn interjection_lead(&self) -> Option<&str> {
        self.setup.interjection_lead.as_deref()
    }

    pub(crate) fn standing(&self, model: &str) -> Vec<String> {
        self.setup
            .instructions
            .iter()
            .map(|instruction| applied(instruction, model))
            .collect()
    }

    pub(crate) fn revision_heading(&self) -> &str {
        &self.setup.revision_heading
    }

    pub(crate) fn agent(
        &self,
        run_context: &RunContext,
        instructions: &[String],
        interjections: &Interjections,
        observer: impl Observer,
    ) -> Result<Agent> {
        let mut agent = Fitting {
            hired: &run_context.hired,
            toolbox: &self.toolbox,
            key: run_context.key.clone(),
            budget: budget(),
            folding: self.setup.folding.clone(),
            keep: 0,
            instructions,
        }
        .agent(observer)
        .cancel_token(run_context.cancel.clone())
        .interjections(interjections.clone());

        if let Some(delegating) = &self.delegating {
            agent = agent.function(delegating.clone());
        }

        agent.build().map_err(setup_error)
    }

    fn new(setup: Arc<Setup>, toolbox: Arc<Toolbox>) -> Result<Assistant> {
        let questions = toolbox
            .questions()
            .cloned()
            .ok_or_else(|| setup_error("toolbox has no question tool"))?;

        let Some(subagent_setup) = setup.subagent_setup.as_ref() else {
            return Ok(Assistant {
                setup: Arc::clone(&setup),
                toolbox,
                questions,
                subagents: None,
                delegating: None,
            });
        };

        let folding = setup
            .folding
            .clone()
            .ok_or_else(|| setup_error("subagents require compaction prompts"))?;
        let subagents = Subagents::new(
            Arc::clone(&toolbox),
            subagent_setup.briefing.clone(),
            folding,
            budget(),
        );
        let delegating =
            delegate::declare(&subagents, &subagent_setup.prompt, &subagent_setup.facts)?;

        Ok(Assistant {
            setup: Arc::clone(&setup),
            toolbox,
            questions,
            subagents: Some(subagents),
            delegating: Some(delegating),
        })
    }
}

impl AssistantBuilder {
    #[must_use]
    pub fn instruction(mut self, text: impl Into<String>) -> AssistantBuilder {
        self.setup.instructions.push(text.into());
        self
    }

    #[must_use]
    pub fn revision_heading(mut self, heading: impl Into<String>) -> AssistantBuilder {
        self.setup.revision_heading = heading.into();
        self
    }

    #[must_use]
    pub fn interjection_lead(mut self, lead: impl Into<String>) -> AssistantBuilder {
        self.setup.interjection_lead = Some(lead.into());
        self
    }

    #[must_use]
    pub fn compaction_prompts(mut self, folding: CompactionPrompts) -> AssistantBuilder {
        self.setup.folding = Some(folding);
        self
    }

    #[must_use]
    pub fn subagents(
        mut self,
        briefing: Vec<String>,
        prompt: &Prompt,
        facts: &Facts,
    ) -> AssistantBuilder {
        self.setup.subagent_setup = Some(SubagentSetup {
            briefing,
            prompt: prompt.clone(),
            facts: facts.clone(),
        });
        self
    }

    pub fn build(mut self) -> Result<Assistant> {
        if let Some(searchable) = self.toolbox.searchable() {
            self.setup.instructions.push(searchable.to_owned());
            if let Some(subagent_setup) = self.setup.subagent_setup.as_mut() {
                subagent_setup.briefing.push(searchable.to_owned());
            }
        }
        if !self.setup.instructions.is_empty() && self.setup.revision_heading.trim().is_empty() {
            return Err(setup_error(
                "standing instructions require a revision heading",
            ));
        }
        Assistant::new(Arc::new(self.setup), Arc::new(self.toolbox))
    }
}
