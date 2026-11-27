use std::sync::Arc;

use rikka_agent::{Deferred, Function, Notices, ToolKitRegistry};
use rikka_file_edit::Files;
use rikka_llm::{ServerTool, Tool};
use rikka_prompt::Facts;

use crate::error::{Error, Result};
use crate::file_access::FileAccess;
use crate::file_history::FileHistory;
use crate::jobs::Jobs;
use crate::limits::most;
use crate::logs::Logs;
use crate::machine::Machine;
use crate::path_instructions::PathInstructions;
use crate::questions::Questions;
use crate::reach::Reach;
use crate::toolbox_builder::ToolboxBuilder;
use crate::{ask, bash, edit, job, tool_search, view, write};
use rikka_tool_docs::{
    ASK, BASH, COMPUTER, DEFERRED, Docs, EDIT, JOB, READING, TOOL_SEARCH, TOOL_SEARCH_APPS, VIEW,
    WRITE,
};

#[derive(Debug)]
pub(crate) struct ToolboxBase {
    docs: Docs,
    facts: Facts,
    server_tools: Vec<ServerTool>,
    aside: Arc<dyn PathInstructions>,
    tools: Vec<Function>,
    deferred: Vec<Function>,
    toolkits: Option<ToolKitRegistry>,
    files: Arc<Files>,
    logs: Logs,
    reach: Reach,
}

pub(crate) struct ToolboxParts {
    pub(crate) functions: Vec<Function>,
    pub(crate) deferred: Deferred,
    pub(crate) searchable: Option<String>,
    pub(crate) toolkits: Option<ToolKitRegistry>,
    pub(crate) machine: Option<Arc<dyn Machine>>,
    pub(crate) questions: Option<Questions>,
    pub(crate) jobs: Jobs,
}

impl ToolboxBase {
    pub(crate) fn new(builder: &ToolboxBuilder) -> Result<ToolboxBase> {
        let files = Files::open(&builder.root).map_err(|error| Error::Rootless {
            path: builder.root.display().to_string(),
            reason: error.to_string(),
        })?;

        let docs = Docs::load()?;
        let facts = docs.facts();
        let logs = Logs::new(files.root(), most(docs.get(BASH)?, "kept_logs")?);
        let server_tools = match builder.web {
            true => ServerTool::ALL.to_vec(),
            false => Vec::new(),
        };
        Ok(ToolboxBase {
            docs,
            facts,
            server_tools,
            aside: Arc::clone(&builder.aside),
            tools: builder.tools.clone(),
            deferred: builder.deferred.clone(),
            toolkits: (!builder.toolkit_sets.is_empty())
                .then(|| ToolKitRegistry::new(builder.toolkit_sets.iter().cloned())),
            files: Arc::new(files),
            logs,
            reach: builder.reach,
        })
    }

    pub(crate) fn server_tools(&self) -> &[ServerTool] {
        &self.server_tools
    }

    pub(crate) fn instantiate(
        &self,
        questions: Option<Questions>,
        machine: Option<Arc<dyn Machine>>,
        history: &FileHistory,
    ) -> Result<ToolboxParts> {
        let docs = &self.docs;
        let facts = &self.facts;

        let bash = docs.get(match self.reach {
            Reach::Working => BASH,
            Reach::Reading => READING,
        })?;
        let jobs = Jobs::new(Notices::new(), most(bash, "retain_bytes")?);

        let shell = bash::Bash {
            files: Arc::clone(&self.files),
            jobs: jobs.clone(),
            logs: self.logs.clone(),
            machine: machine.clone(),
            reach: self.reach,
        };
        let mut functions = vec![
            bash::declare(shell, bash, facts)?,
            job::declare(&jobs, docs.get(JOB)?, facts)?,
            view::declare(&self.files, docs.get(VIEW)?, facts)?,
        ];
        if self.reach == Reach::Working {
            if let Some(machine) = &machine {
                functions.push(machine.tool(docs.get(machine.document())?, facts)?);
            }
            let access = Arc::new(FileAccess {
                files: Arc::clone(&self.files),
                history: history.clone(),
                aside: Arc::clone(&self.aside),
            });
            functions.extend([
                write::declare(&access, docs.get(WRITE)?, facts)?,
                edit::declare(&access, docs.get(EDIT)?, facts)?,
            ]);
        }

        if let Some(questions) = &questions {
            functions.push(ask::declare(questions, docs.get(ASK)?, facts)?);
        }
        functions.extend(self.tools.iter().cloned());

        let deferred = Deferred::new();
        if self.reach == Reach::Working {
            let described = machine
                .as_ref()
                .map_or(COMPUTER, |machine| machine.document());
            for name in docs.deferred(described) {
                deferred.defer(name);
            }
        }

        let mut searchable: Vec<Tool> = functions
            .iter()
            .filter(|function| deferred.is_hidden(function.name()))
            .map(|function| function.tool().clone())
            .collect();

        for function in self.deferred.iter().cloned() {
            deferred.defer(function.name());
            searchable.push(function.tool().clone());
            functions.push(function);
        }

        let mut listed = None;
        if !searchable.is_empty() || self.toolkits.is_some() {
            let document = match self.toolkits {
                Some(_) => TOOL_SEARCH_APPS,
                None => TOOL_SEARCH,
            };
            if !searchable.is_empty() {
                let mut facts = facts.clone();
                facts.set("deferred_tools", names(&searchable));
                listed = Some(docs.get(DEFERRED)?.body(&facts)?);
            }
            functions.push(tool_search::declare(
                &deferred,
                searchable,
                self.toolkits.clone(),
                docs.get(document)?,
                facts,
            )?);
        }

        Ok(ToolboxParts {
            functions,
            deferred,
            searchable: listed,
            toolkits: self.toolkits.clone(),
            machine,
            questions,
            jobs,
        })
    }
}

fn names(searchable: &[Tool]) -> String {
    searchable
        .iter()
        .map(Tool::name)
        .collect::<Vec<&str>>()
        .join("\n")
}
