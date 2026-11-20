use std::sync::Arc;

use serde_json::Value;

use crate::handler::{Answer, Handler};
use crate::tool::Tool;

#[derive(Clone)]
pub struct Function {
    tool: Tool,
    handler: Arc<dyn Handler>,
    lasting: bool,
}

impl Function {
    #[must_use]
    pub fn new(tool: Tool, handler: impl Handler) -> Function {
        Function {
            tool,
            handler: Arc::new(handler),
            lasting: false,
        }
    }

    #[must_use]
    pub fn survives_compaction(mut self) -> Function {
        self.lasting = true;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.tool.name()
    }

    #[must_use]
    pub fn tool(&self) -> &Tool {
        &self.tool
    }

    #[must_use]
    pub fn is_lasting(&self) -> bool {
        self.lasting
    }

    pub fn call(&self, input: Value) -> Answer {
        self.handler.call(input)
    }
}

impl std::fmt::Debug for Function {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Function")
            .field("name", &self.tool.name())
            .field("lasting", &self.lasting)
            .finish_non_exhaustive()
    }
}
