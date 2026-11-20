use std::collections::BTreeSet;

use rikka_llm::Value;
use serde_json::Map;

use crate::error::{Error, Result};
use crate::naming;
use crate::protocol::Prompt as Offered;

const BETWEEN: &str = ":";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    server: String,
    command: String,
    name: String,
    title: Option<String>,
    description: Option<String>,
    arguments: Vec<PromptArgument>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptArgument {
    name: String,
    title: Option<String>,
    description: Option<String>,
    required: bool,
}

impl Prompt {
    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[must_use]
    pub fn arguments(&self) -> &[PromptArgument] {
        &self.arguments
    }

    pub(crate) fn filled(&self, values: &[String]) -> Result<Map<String, Value>> {
        if self.arguments.is_empty() && !values.is_empty() {
            return Err(Error::Unasked(self.command.clone()));
        }
        let mut filled = Map::new();
        for (at, argument) in self.arguments.iter().enumerate() {
            let last = at + 1 == self.arguments.len();
            let value = match last {
                true => values.get(at..).map(|rest| rest.join(" ")),
                false => values.get(at).cloned(),
            };
            match value.filter(|value| !value.is_empty()) {
                Some(value) => {
                    filled.insert(argument.name.clone(), Value::String(value));
                }
                None if argument.required => {
                    return Err(Error::Unfilled {
                        command: self.command.clone(),
                        argument: argument.name.clone(),
                    });
                }
                None => {}
            }
        }
        Ok(filled)
    }
}

impl PromptArgument {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[must_use]
    pub fn is_required(&self) -> bool {
        self.required
    }
}

pub(crate) fn listed(server: &str, label: &str, offered: Vec<Offered>) -> Vec<Prompt> {
    let mut used_commands = BTreeSet::new();
    offered
        .into_iter()
        .map(|prompt| Prompt {
            server: server.to_owned(),
            command: naming::unique(
                format!("{label}{BETWEEN}{}", naming::label(&prompt.name)),
                &mut used_commands,
            ),
            title: worded(prompt.title),
            description: worded(prompt.description),
            arguments: prompt
                .arguments
                .unwrap_or_default()
                .into_iter()
                .map(|argument| PromptArgument {
                    name: argument.name,
                    title: worded(argument.title),
                    description: worded(argument.description),
                    required: argument.required.unwrap_or(false),
                })
                .collect(),
            name: prompt.name,
        })
        .collect()
}

pub(crate) fn worded(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}
