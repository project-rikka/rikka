use std::fmt;

use serde_json::Value;

use crate::server_tool::ServerTool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedPart {
    Call {
        id: String,
        tool: ServerTool,
        input: Value,
    },
    Result {
        id: String,
        tool: ServerTool,
        outcome: HostedOutcome,
    },
}

impl HostedPart {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            HostedPart::Call { id, .. } | HostedPart::Result { id, .. } => id,
        }
    }

    #[must_use]
    pub fn tool(&self) -> ServerTool {
        match self {
            HostedPart::Call { tool, .. } | HostedPart::Result { tool, .. } => *tool,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedOutcome {
    Found(Vec<Source>),
    Fetched(Source),
    Failed(String),
}

impl HostedOutcome {
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, HostedOutcome::Failed(_))
    }
}

impl fmt::Display for HostedOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostedOutcome::Found(sources) if sources.is_empty() => {
                formatter.write_str("no results")
            }
            HostedOutcome::Found(sources) => {
                for (index, source) in sources.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str("\n")?;
                    }
                    write!(formatter, "{source}")?;
                }
                Ok(())
            }
            HostedOutcome::Fetched(source) => write!(formatter, "{source}"),
            HostedOutcome::Failed(code) if code.is_empty() => formatter.write_str("failed"),
            HostedOutcome::Failed(code) => formatter.write_str(&code.replace('_', " ")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    url: String,
    title: Option<String>,
    date: Option<String>,
}

impl Source {
    pub(crate) fn new(url: String, title: Option<String>, date: Option<String>) -> Source {
        Source { url, title, date }
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(title) = &self.title {
            write!(formatter, "{title} — ")?;
        }
        formatter.write_str(&self.url)?;
        if let Some(date) = &self.date {
            write!(formatter, " ({date})")?;
        }
        Ok(())
    }
}
