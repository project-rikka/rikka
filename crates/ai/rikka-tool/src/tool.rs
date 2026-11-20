use serde_json::Value;

use crate::error::{Error, Result};
use crate::schema::Schema;
use crate::tool_builder::ToolBuilder;

#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    name: String,
    description: String,
    schema: Value,
    strict: bool,
    cached: bool,
    deferred: bool,
}

impl Tool {
    #[expect(
        clippy::new_ret_no_self,
        reason = "a caller names the tool they are declaring, not the builder that gets them there"
    )]
    pub fn new(name: impl Into<String>) -> ToolBuilder {
        ToolBuilder::new(name)
    }

    pub fn from_schema(name: impl Into<String>, schema: Value) -> ToolBuilder {
        ToolBuilder::from_schema(name, schema)
    }

    pub(crate) fn build(
        name: String,
        description: String,
        mut schema: Schema,
        optional: Vec<String>,
        strict: bool,
        cached: bool,
    ) -> Result<Tool> {
        for name in &optional {
            if !schema.declares(name) {
                return Err(Error::Undeclared(name.clone()));
            }
            schema.demote(name);
        }

        if strict {
            schema.close();
        }

        Ok(Tool {
            name,
            description,
            schema: schema.into_value(),
            strict,
            cached,
            deferred: false,
        })
    }

    pub(crate) fn from_raw(
        name: String,
        description: String,
        schema: Value,
        strict: bool,
        cached: bool,
    ) -> Tool {
        Tool {
            name,
            description,
            schema,
            strict,
            cached,
            deferred: false,
        }
    }

    #[must_use]
    pub fn deferred(mut self) -> Tool {
        self.deferred = true;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn schema(&self) -> &Value {
        &self.schema
    }

    #[must_use]
    pub fn is_strict(&self) -> bool {
        self.strict
    }

    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.cached
    }

    #[must_use]
    pub fn is_deferred(&self) -> bool {
        self.deferred
    }
}
