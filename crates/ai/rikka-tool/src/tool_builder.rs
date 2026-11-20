use serde_json::Value;

use crate::error::{Error, Result};
use crate::schema::Schema;
use crate::tool::Tool;

#[must_use]
#[derive(Debug, Clone)]
pub struct ToolBuilder {
    name: String,
    description: String,
    schema: Schema,
    raw: Option<Value>,
    optional: Vec<String>,
    declared: bool,
    strict: bool,
    cached: bool,
}

impl ToolBuilder {
    pub(crate) fn new(name: impl Into<String>) -> ToolBuilder {
        ToolBuilder {
            name: name.into(),
            description: String::new(),
            schema: Schema::object(),
            raw: None,
            optional: Vec::new(),
            declared: false,
            strict: false,
            cached: false,
        }
    }

    pub(crate) fn from_schema(name: impl Into<String>, schema: Value) -> ToolBuilder {
        ToolBuilder {
            raw: Some(schema),
            ..ToolBuilder::new(name)
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn string(self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.param(name, Schema::string().describe(description))
    }

    pub fn integer(self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.param(name, Schema::integer().describe(description))
    }

    pub fn number(self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.param(name, Schema::number().describe(description))
    }

    pub fn boolean(self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.param(name, Schema::boolean().describe(description))
    }

    pub fn enumerated<T: Into<Value>>(
        self,
        name: impl Into<String>,
        values: impl IntoIterator<Item = T>,
        description: impl Into<String>,
    ) -> Self {
        self.param(
            name,
            Schema::string().enumerate(values).describe(description),
        )
    }

    pub fn param(mut self, name: impl Into<String>, schema: Schema) -> Self {
        self.schema = std::mem::take(&mut self.schema).param(name, schema);
        self.declared = true;
        self
    }

    pub fn optional(mut self, name: impl Into<String>) -> Self {
        self.optional.push(name.into());
        self
    }

    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub fn cached(mut self) -> Self {
        self.cached = true;
        self
    }

    pub fn build(self) -> Result<Tool> {
        match self.raw {
            None => Tool::build(
                self.name,
                self.description,
                self.schema,
                self.optional,
                self.strict,
                self.cached,
            ),
            Some(_) if self.declared || !self.optional.is_empty() => {
                Err(Error::Overdeclared(self.name))
            }
            Some(raw) => Ok(Tool::from_raw(
                self.name,
                self.description,
                raw,
                self.strict,
                self.cached,
            )),
        }
    }
}
