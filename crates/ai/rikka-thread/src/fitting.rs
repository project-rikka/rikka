use rikka_agent::{Agent, AgentBuilder, Budget, Compaction, CompactionPrompts, Observer};
use rikka_llm::Ttl;
use rikka_llm_accounts::ConnectedModel;
use rikka_tools::Toolbox;

pub(crate) struct Fitting<'a> {
    pub(crate) hired: &'a ConnectedModel,
    pub(crate) toolbox: &'a Toolbox,
    pub(crate) key: String,
    pub(crate) budget: Budget,
    pub(crate) folding: Option<CompactionPrompts>,
    pub(crate) keep: usize,
    pub(crate) instructions: &'a [String],
}

impl Fitting<'_> {
    pub(crate) fn agent(self, observer: impl Observer) -> AgentBuilder {
        let hired = self.hired;
        let mut agent = Agent::new(hired.model().clone())
            .functions(self.toolbox.functions().iter().cloned())
            .server_tools(self.toolbox.server_tools().iter().copied())
            .deferred(self.toolbox.deferred().clone())
            .budget(self.budget)
            .notices(self.toolbox.jobs().notices().clone())
            .ttl(Ttl::Long)
            .cache_key(self.key)
            .observer(observer);

        if let Some(rack) = self.toolbox.toolkits() {
            agent = agent.toolkits(rack.clone());
        }
        if let Some(folding) = self.folding {
            let jobs = self.toolbox.jobs().clone();
            agent = agent.compaction(
                Compaction::new(hired.context_window(), folding)
                    .output(hired.max_output())
                    .keep(self.keep)
                    .restatement(move || jobs.summary()),
            );
        }
        if let Some(effort) = hired.effort() {
            agent = agent.effort(effort);
        }
        if hired.info().is_some_and(|info| !info.vision()) {
            agent = agent.without_vision();
        }
        for instruction in self.instructions {
            agent = agent.instructions(instruction.clone());
        }

        agent
    }
}

pub(crate) fn applied(instruction: &str, model: &str) -> String {
    match instruction.contains("{{model}}") {
        true => instruction.replace("{{model}}", model),
        false => instruction.to_owned(),
    }
}
