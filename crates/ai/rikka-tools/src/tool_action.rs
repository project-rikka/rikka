use rikka_llm::{ServerTool, Value};

use crate::look::{Look, look};
use rikka_tool_docs::Naming;
use rikka_tool_docs::{ASK, BASH, COMPUTER, EDIT, JOB, TOOL_SEARCH, VIEW, WRITE};

#[derive(Debug, PartialEq, Eq)]
pub enum ToolAction<'a> {
    Command {
        command: &'a str,
    },
    Inspect {
        command: &'a str,
        look: Look,
    },
    View {
        path: &'a str,
    },
    Write {
        path: &'a str,
        content: &'a str,
    },
    Edit {
        path: &'a str,
    },
    Ask {
        questions: &'a [Value],
    },
    Job {
        action: &'a str,
        id: Option<u64>,
    },
    Action {
        action: &'a str,
        text: Option<&'a str>,
    },
    Operate(Operation<'a>),
    Search {
        query: &'a str,
    },
    Fetch {
        url: &'a str,
    },
    Tool {
        stem: &'static str,
        input: &'a Value,
    },
    Unknown,
}

impl<'a> ToolAction<'a> {
    #[must_use]
    pub fn classify(naming: &Naming, tool: &str, input: &'a Value) -> ToolAction<'a> {
        if let Some(server) = ServerTool::from_name(tool) {
            return ToolAction::hosted(server, input);
        }
        match naming.stem(tool) {
            Some(BASH) => ToolAction::ran(string_field(input, "command")),
            Some(VIEW) => ToolAction::View {
                path: string_field(input, "path"),
            },
            Some(WRITE) => ToolAction::Write {
                path: string_field(input, "path"),
                content: string_field(input, "content"),
            },
            Some(EDIT) => ToolAction::Edit {
                path: string_field(input, "path"),
            },
            Some(JOB) => ToolAction::Job {
                action: string_field(input, "action"),
                id: input.get("id").and_then(Value::as_u64),
            },
            Some(COMPUTER) => ToolAction::Operate(Operation::from_input(input)),
            Some(ASK | TOOL_SEARCH) | None => ToolAction::guessed(input),
            Some(stem) => ToolAction::Tool { stem, input },
        }
    }

    #[must_use]
    pub fn name<'b>(&'b self, tool: &'b str) -> &'b str {
        match (self, ServerTool::from_name(tool)) {
            (ToolAction::Inspect { look, .. }, _) => look.inspection().name(),
            (_, Some(server)) => server.label(),
            (_, None) => tool,
        }
    }

    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        match self {
            ToolAction::Command { command } => Some(command),
            ToolAction::Inspect { look, .. } => look.subject(),
            ToolAction::View { path }
            | ToolAction::Write { path, .. }
            | ToolAction::Edit { path } => Some(path),
            ToolAction::Job { action, .. } | ToolAction::Action { action, .. } => Some(action),
            ToolAction::Search { query } => Some(query),
            ToolAction::Fetch { url } => Some(url),
            ToolAction::Ask { .. }
            | ToolAction::Operate(_)
            | ToolAction::Tool { .. }
            | ToolAction::Unknown => None,
        }
    }

    fn hosted(server: ServerTool, input: &'a Value) -> ToolAction<'a> {
        match server {
            ServerTool::WebSearch => ToolAction::Search {
                query: string_field(input, "query"),
            },
            ServerTool::WebFetch => ToolAction::Fetch {
                url: string_field(input, "url"),
            },
        }
    }

    fn ran(command: &'a str) -> ToolAction<'a> {
        match look(command) {
            Some(look) => ToolAction::Inspect { command, look },
            None => ToolAction::Command { command },
        }
    }

    fn guessed(input: &'a Value) -> ToolAction<'a> {
        if let Some(action) = input.get("action").and_then(Value::as_str) {
            let text = input.get("text").and_then(Value::as_str);
            return ToolAction::Action { action, text };
        }
        if let Some(questions) = input.get("questions").and_then(Value::as_array) {
            return ToolAction::Ask { questions };
        }
        if let Some(query) = input.get("query").and_then(Value::as_str) {
            return ToolAction::Search { query };
        }
        if let Some(url) = input.get("url").and_then(Value::as_str) {
            return ToolAction::Fetch { url };
        }
        ToolAction::Unknown
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Operation<'a> {
    actions: &'a [Value],
}

impl<'a> Operation<'a> {
    fn from_input(input: &'a Value) -> Operation<'a> {
        Operation {
            actions: input
                .get("actions")
                .and_then(Value::as_array)
                .map_or(&[], Vec::as_slice),
        }
    }
}

impl std::fmt::Display for Operation<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let named = self
            .actions
            .iter()
            .filter_map(|step| Some((step.get("action")?.as_str()?, step.get("text"))));
        for (index, (action, text)) in named.enumerate() {
            if index > 0 {
                formatter.write_str(" · ")?;
            }
            match text.and_then(Value::as_str) {
                Some(text) => write!(formatter, "{action} {text}")?,
                None => formatter.write_str(action)?,
            }
        }
        Ok(())
    }
}

fn string_field<'a>(input: &'a Value, key: &str) -> &'a str {
    input.get(key).and_then(Value::as_str).unwrap_or_default()
}
