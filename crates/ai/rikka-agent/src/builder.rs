use std::sync::Arc;

use rikka_llm::{Cache, CancelToken, Effort, Format, Model, Reveal, ServerTool, Text, Tool, Ttl};

use crate::agent::Agent;
use crate::budget::Budget;
use crate::compaction::Compaction;
use crate::deferred::Deferred;
use crate::delegate::Delegate;
use crate::error::{Error, Result};
use crate::event::Observer;
use crate::inner::Inner;
use crate::interjections::Interjections;
use crate::notices::Notices;
use rikka_tool::Function;
use rikka_tool::Handler;
use rikka_tool::ToolKitRegistry;

#[must_use]
pub struct AgentBuilder {
    model: Model,
    name: Option<String>,
    description: Option<String>,
    instructions: Vec<Text>,
    functions: Vec<Function>,
    server_tools: Vec<ServerTool>,
    deferred: Deferred,
    rack: Option<ToolKitRegistry>,
    budget: Budget,
    compaction: Option<Compaction>,
    effort: Option<Effort>,
    reveal: Reveal,
    format: Option<Format>,
    cache: Cache,
    ttl: Ttl,
    cache_key: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    parallel_tools: bool,
    observer: Option<Arc<dyn Observer>>,
    cancel: Option<CancelToken>,
    notices: Notices,
    waits: bool,
    interjections: Interjections,
    vision: bool,
    error: Option<Error>,
}

impl AgentBuilder {
    pub(crate) fn new(model: Model) -> AgentBuilder {
        AgentBuilder {
            model,
            name: None,
            description: None,
            instructions: Vec::new(),
            functions: Vec::new(),
            server_tools: Vec::new(),
            deferred: Deferred::default(),
            rack: None,
            budget: Budget::default(),
            compaction: None,
            effort: None,
            reveal: Reveal::Hidden,
            format: None,
            cache: Cache::Conversation,
            ttl: Ttl::Short,
            cache_key: None,
            max_tokens: None,
            temperature: None,
            parallel_tools: true,
            observer: None,
            cancel: None,
            notices: Notices::new(),
            waits: false,
            interjections: Interjections::new(),
            vision: true,
            error: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn instructions(mut self, instructions: impl Into<Text>) -> Self {
        self.instructions.push(instructions.into());
        self
    }

    pub fn tool(self, tool: Tool, handler: impl Handler) -> Self {
        self.function(Function::new(tool, handler))
    }

    pub fn function(mut self, function: Function) -> Self {
        self.functions.push(function);
        self
    }

    pub fn functions(mut self, functions: impl IntoIterator<Item = Function>) -> Self {
        self.functions.extend(functions);
        self
    }

    pub fn server_tools(mut self, tools: impl IntoIterator<Item = ServerTool>) -> Self {
        self.server_tools.extend(tools);
        self
    }

    pub fn deferred(mut self, deferred: Deferred) -> Self {
        self.deferred = deferred;
        self
    }

    pub fn toolkits(mut self, rack: ToolKitRegistry) -> Self {
        self.rack = Some(rack);
        self
    }

    pub fn subagent(mut self, agent: Agent) -> Self {
        match agent.as_tool() {
            Ok(tool) => self
                .functions
                .push(Function::new(tool, Delegate::new(agent))),
            Err(error) => {
                self.error.get_or_insert(error);
            }
        }
        self
    }

    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    pub fn compaction(mut self, compaction: Compaction) -> Self {
        self.compaction = Some(compaction);
        self
    }

    pub fn effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    pub fn reveal(mut self, reveal: Reveal) -> Self {
        self.reveal = reveal;
        self
    }

    pub fn format(mut self, format: Format) -> Self {
        self.format = Some(format);
        self
    }

    pub fn cache(mut self, cache: Cache) -> Self {
        self.cache = cache;
        self
    }

    pub fn ttl(mut self, ttl: Ttl) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn cache_key(mut self, key: impl Into<String>) -> Self {
        self.cache_key = Some(key.into());
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn parallel_tools(mut self, parallel: bool) -> Self {
        self.parallel_tools = parallel;
        self
    }

    pub fn observer(mut self, observer: impl Observer) -> Self {
        self.observer = Some(Arc::new(observer));
        self
    }

    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.cancel = Some(token);
        self
    }

    pub fn notices(mut self, notices: Notices) -> Self {
        self.notices = notices;
        self
    }

    pub fn wait_for_notices(mut self, waits: bool) -> Self {
        self.waits = waits;
        self
    }

    pub fn interjections(mut self, interjections: Interjections) -> Self {
        self.interjections = interjections;
        self
    }

    pub fn without_vision(mut self) -> Self {
        self.vision = false;
        self
    }

    pub fn build(mut self) -> Result<Agent> {
        if let Some(error) = self.error.take() {
            return Err(error);
        }

        for (index, function) in self.functions.iter().enumerate() {
            if self.functions[..index]
                .iter()
                .any(|earlier| earlier.name() == function.name())
            {
                return Err(Error::DuplicateTool(function.name().to_owned()));
            }
        }
        if let Some(function) = self.functions.iter().find(|function| {
            self.server_tools
                .iter()
                .any(|tool| tool.name() == function.name())
        }) {
            return Err(Error::DuplicateTool(function.name().to_owned()));
        }

        Ok(Agent::from_inner(Inner {
            model: self.model,
            name: self.name,
            description: self.description,
            instructions: self.instructions,
            functions: self.functions,
            server_tools: self.server_tools,
            deferred: self.deferred,
            rack: self.rack,
            budget: self.budget,
            compaction: self.compaction,
            effort: self.effort,
            reveal: self.reveal,
            format: self.format,
            cache: self.cache,
            ttl: self.ttl,
            cache_key: self.cache_key,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            parallel_tools: self.parallel_tools,
            observer: self.observer,
            cancel: self.cancel,
            notices: self.notices,
            waits: self.waits,
            interjections: self.interjections,
            vision: self.vision,
        }))
    }
}

impl std::fmt::Debug for AgentBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentBuilder")
            .field("name", &self.name)
            .field("model", &self.model.name())
            .field("tools", &self.functions.len())
            .finish_non_exhaustive()
    }
}
