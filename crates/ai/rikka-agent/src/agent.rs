use std::sync::Arc;

use rikka_llm::{IntoContent, Message, Model, Tool};

use crate::builder::AgentBuilder;
use crate::error::{Error, Result};
use crate::inner::Inner;
use crate::outcome::Outcome;
use crate::session::Session;

#[derive(Debug, Clone)]
pub struct Agent {
    inner: Arc<Inner>,
}

impl Agent {
    #[expect(
        clippy::new_ret_no_self,
        reason = "a caller names the agent they are declaring, not the builder that gets them there"
    )]
    pub fn new(model: Model) -> AgentBuilder {
        AgentBuilder::new(model)
    }

    pub(crate) fn from_inner(inner: Inner) -> Agent {
        Agent {
            inner: Arc::new(inner),
        }
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    #[must_use]
    pub fn model(&self) -> &Model {
        &self.inner.model
    }

    pub async fn run(&self, task: impl IntoContent) -> Result<Outcome> {
        self.session().send(task).await
    }

    #[must_use]
    pub fn session(&self) -> Session {
        Session::new(self.clone())
    }

    #[must_use]
    pub fn resume(&self, history: Vec<Message>) -> Session {
        Session::resume(self.clone(), history)
    }

    pub(crate) fn inner(&self) -> &Inner {
        &self.inner
    }

    pub(crate) fn as_tool(&self) -> Result<Tool> {
        let (Some(name), Some(description)) = (self.name(), self.description()) else {
            return Err(Error::Anonymous);
        };

        Tool::new(name)
            .description(description)
            .string(
                "task",
                "What this agent should do, in full — it sees nothing else",
            )
            .build()
            .map_err(|error| Error::Model(error.into()))
    }
}
