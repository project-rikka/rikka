use rikka_llm::Value;

use crate::agent::Agent;
use rikka_tool::Reply;
use rikka_tool::{Answer, Handler};

#[derive(Debug)]
pub(crate) struct Delegate {
    agent: Agent,
}

impl Delegate {
    pub(crate) fn new(agent: Agent) -> Delegate {
        Delegate { agent }
    }
}

impl Handler for Delegate {
    fn call(&self, input: Value) -> Answer {
        let agent = self.agent.clone();
        let task = input
            .get("task")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();

        Box::pin(async move {
            if task.is_empty() {
                return Err("the task was empty".to_owned());
            }

            match agent.run(task).await {
                Ok(outcome) => Ok(Reply::new(outcome.text()).with_usage(outcome.usage())),
                Err(error) => Err(error.to_string()),
            }
        })
    }
}
