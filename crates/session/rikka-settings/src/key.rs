use std::fmt;
use std::sync::Arc;

use crate::constraint::Constraint;
use crate::domain::is_valid_key;
use crate::value::{Value, ValueType, Values};

type Check = Arc<dyn Fn(&Value) -> Result<(), String> + Send + Sync>;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Key {
    name: Box<str>,
    default: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<Box<str>>,
    #[serde(default, skip_serializing_if = "Constraint::is_any")]
    constraint: Constraint,
    #[serde(skip)]
    check: Option<Check>,
}

impl Key {
    #[must_use]
    pub fn bool(name: impl Into<String>, default: bool) -> Self {
        Self::declare(name, Value::Bool(default))
    }

    #[must_use]
    pub fn int(name: impl Into<String>, default: i64) -> Self {
        Self::declare(name, Value::Int(default))
    }

    #[must_use]
    pub fn float(name: impl Into<String>, default: f64) -> Self {
        Self::declare(name, Value::Float(default))
    }

    #[must_use]
    pub fn string(name: impl Into<String>, default: impl Into<String>) -> Self {
        Self::declare(name, Value::String(default.into()))
    }

    #[must_use]
    pub fn list(name: impl Into<String>, default: impl IntoIterator<Item = Value>) -> Self {
        Self::declare(name, Value::List(default.into_iter().collect()))
    }

    #[must_use]
    pub fn table(name: impl Into<String>, default: Values) -> Self {
        Self::declare(name, Value::Table(default))
    }

    fn declare(name: impl Into<String>, default: Value) -> Self {
        Self {
            name: name.into().into_boxed_str(),
            default,
            summary: None,
            constraint: Constraint::Any,
            check: None,
        }
    }

    #[must_use]
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into().into_boxed_str());
        self
    }

    #[must_use]
    pub fn range(mut self, range: impl Into<Constraint>) -> Self {
        self.constraint = range.into();
        self
    }

    #[must_use]
    pub fn choices<V: Into<Value>>(mut self, choices: impl IntoIterator<Item = V>) -> Self {
        self.constraint = Constraint::Choices(choices.into_iter().map(Into::into).collect());
        self
    }

    #[must_use]
    pub fn checked_with(
        mut self,
        check: impl Fn(&Value) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.check = Some(Arc::new(check));
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn default_value(&self) -> &Value {
        &self.default
    }

    #[must_use]
    pub fn summary_text(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    #[must_use]
    pub fn constraint(&self) -> &Constraint {
        &self.constraint
    }

    #[must_use]
    pub fn value_type(&self) -> ValueType {
        self.default.value_type()
    }

    pub fn accepts(&self, value: Value) -> Result<Value, String> {
        let value = value.coerced(self.value_type());
        let found = value.value_type();
        if found != self.value_type() {
            return Err(format!("expected {}, found {found}", self.value_type()));
        }
        self.constraint.check(&value)?;
        if let Some(check) = &self.check {
            check(&value)?;
        }
        Ok(value)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !is_valid_key(self.name()) {
            return Err(format!(
                "`{}` is not a valid setting name: expected ASCII letters, digits, `-` or `_`",
                self.name()
            ));
        }
        self.accepts(self.default.clone())
            .map(|_| ())
            .map_err(|reason| format!("the default for `{}` {reason}", self.name()))
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Key")
            .field("name", &self.name)
            .field("default", &self.default)
            .field("constraint", &self.constraint)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.default == other.default
            && self.summary == other.summary
            && self.constraint == other.constraint
    }
}
