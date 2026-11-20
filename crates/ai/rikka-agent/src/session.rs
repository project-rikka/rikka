use rikka_llm::{IntoContent, Message, Request, Usage};

use crate::agent::Agent;
use crate::error::Result;
use crate::gauge::Gauge;
use crate::outcome::Outcome;

#[derive(Debug, Clone)]
pub struct Session {
    agent: Agent,
    request: Request,
    usage: Usage,
    gauge: Gauge,
}

impl Session {
    pub(crate) fn new(agent: Agent) -> Session {
        Session::resume(agent, Vec::new())
    }

    pub(crate) fn resume(agent: Agent, history: Vec<Message>) -> Session {
        Session {
            request: agent.inner().request(history),
            agent,
            usage: Usage::default(),
            gauge: Gauge::default(),
        }
    }

    #[must_use]
    pub fn filled(mut self, tokens: u64) -> Session {
        self.gauge = Gauge::counted(tokens, self.request.messages().len());
        self
    }

    pub async fn send(&mut self, message: impl IntoContent) -> Result<Outcome> {
        let from = self.request.messages().len();
        self.request.push(Message::user(message));

        let outcome = self
            .agent
            .inner()
            .run(&mut self.request, from, &mut self.gauge)
            .await?;
        self.usage += outcome.usage();
        Ok(outcome)
    }

    pub async fn compact(&mut self, focus: &str) -> Result<Usage> {
        let spent = self.agent.inner().compact(&mut self.request, focus).await?;
        self.gauge = Gauge::default();
        self.usage += spent;
        Ok(spent)
    }

    pub fn push(&mut self, message: Message) {
        self.request.push(message);
    }

    #[must_use]
    pub fn history(&self) -> &[Message] {
        self.request.messages()
    }

    #[must_use]
    pub fn usage(&self) -> Usage {
        self.usage
    }

    #[must_use]
    pub fn agent(&self) -> &Agent {
        &self.agent
    }

    pub fn clear(&mut self) {
        self.set_history(Vec::new());
        self.usage = Usage::default();
        self.gauge = Gauge::default();
    }

    pub fn set_history(&mut self, history: Vec<Message>) {
        self.request.set_messages(history);
        self.gauge = Gauge::default();
    }
}
