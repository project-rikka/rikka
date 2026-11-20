use serde_json::Value;

use rikka_tool::Schema;

#[derive(Debug, Clone, PartialEq)]
pub struct Format {
    name: String,
    schema: Value,
    strict: bool,
}

impl Format {
    #[must_use]
    pub fn new(name: impl Into<String>, schema: Schema) -> Format {
        Format {
            name: name.into(),
            schema: schema.into_value(),
            strict: true,
        }
    }

    #[must_use]
    pub fn from_schema(name: impl Into<String>, schema: Value) -> Format {
        Format {
            name: name.into(),
            schema,
            strict: true,
        }
    }

    #[must_use]
    pub fn lenient(mut self) -> Self {
        self.strict = false;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn schema(&self) -> Value {
        let mut schema = Schema::raw(self.schema.clone());
        if self.strict {
            schema.close();
        }
        schema.into_value()
    }

    #[must_use]
    pub fn is_strict(&self) -> bool {
        self.strict
    }
}
