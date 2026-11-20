use std::path::PathBuf;
use std::sync::Arc;

use rikka_agent::{Deferred, Function, ToolKitRegistry};
use rikka_file_edit::History;
use rikka_llm::ServerTool;

use crate::error::Result;
use crate::file_history::FileHistory;
use crate::jobs::Jobs;
use crate::machine::Machine;
use crate::questions::Questions;
use crate::tool_search::{self, ToolKitEntry};
use crate::toolbox_base::ToolboxBase;
use crate::toolbox_builder::ToolboxBuilder;

#[derive(Debug)]
pub struct Toolbox {
    functions: Vec<Function>,
    deferred: Deferred,
    searchable: Option<String>,
    toolkits: Option<ToolKitRegistry>,
    machine: Option<Arc<dyn Machine>>,
    questions: Option<Questions>,
    jobs: Jobs,
    history: FileHistory,
    base: Arc<ToolboxBase>,
}

impl Toolbox {
    #[must_use]
    pub fn builder(root: impl Into<PathBuf>) -> ToolboxBuilder {
        ToolboxBuilder::new(root)
    }

    pub(crate) fn from_builder(builder: ToolboxBuilder) -> Result<Toolbox> {
        let base = Arc::new(ToolboxBase::new(&builder)?);
        Toolbox::new(
            base,
            Some(Questions::new()),
            builder.machine,
            FileHistory::default(),
        )
    }

    pub fn fresh(&self) -> Result<Toolbox> {
        Toolbox::new(
            Arc::clone(&self.base),
            Some(Questions::new()),
            self.machine.as_ref().map(|machine| machine.isolated()),
            FileHistory::default(),
        )
    }

    pub fn for_subagent(&self) -> Result<Toolbox> {
        Toolbox::new(
            Arc::clone(&self.base),
            None,
            self.machine.clone(),
            self.history.clone(),
        )
    }

    pub fn keep_files_in(&self, history: History) {
        self.history.keep_in(history);
    }

    #[must_use]
    pub fn history(&self) -> Option<History> {
        self.history.history()
    }

    #[must_use]
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    #[must_use]
    pub fn server_tools(&self) -> &[ServerTool] {
        self.base.server_tools()
    }

    #[must_use]
    pub fn deferred(&self) -> &Deferred {
        &self.deferred
    }

    #[must_use]
    pub fn searchable(&self) -> Option<&str> {
        self.searchable.as_deref()
    }

    #[must_use]
    pub fn toolkits(&self) -> Option<&ToolKitRegistry> {
        self.toolkits.as_ref()
    }

    #[must_use]
    pub fn toolkit(&self, name: &str) -> Option<ToolKitEntry> {
        let kit = self.toolkits.as_ref()?.kit(name)?;
        Some(tool_search::peek(&self.deferred, kit))
    }

    #[must_use]
    pub fn open_toolkit(&self, name: &str) -> Option<ToolKitEntry> {
        let kit = self.toolkits.as_ref()?.kit(name)?;
        Some(tool_search::open(&self.deferred, kit))
    }

    #[must_use]
    pub fn questions(&self) -> Option<&Questions> {
        self.questions.as_ref()
    }

    #[must_use]
    pub fn machine(&self) -> Option<&dyn Machine> {
        self.machine.as_deref()
    }

    #[must_use]
    pub fn jobs(&self) -> &Jobs {
        &self.jobs
    }

    fn new(
        base: Arc<ToolboxBase>,
        questions: Option<Questions>,
        machine: Option<Arc<dyn Machine>>,
        history: FileHistory,
    ) -> Result<Toolbox> {
        let parts = base.instantiate(questions, machine, &history)?;
        Ok(Toolbox {
            functions: parts.functions,
            deferred: parts.deferred,
            searchable: parts.searchable,
            toolkits: parts.toolkits,
            machine: parts.machine,
            questions: parts.questions,
            jobs: parts.jobs,
            history,
            base,
        })
    }
}
