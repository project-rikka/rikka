use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::rules::{Environment, Rule, allowed};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Values {
    One(String),
    Many(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional {
        #[serde(default)]
        rules: Vec<Rule>,
        value: Values,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

impl Arguments {
    pub(crate) fn extend(&mut self, child: Arguments) {
        self.game.extend(child.game);
        self.jvm.extend(child.jvm);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Substitutions(BTreeMap<String, String>);

impl Substitutions {
    #[must_use]
    pub fn new() -> Substitutions {
        Substitutions::default()
    }

    #[must_use]
    pub fn with(mut self, name: &str, value: impl Into<String>) -> Substitutions {
        self.0.insert(name.to_owned(), value.into());
        self
    }

    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        self.0.insert(name.to_owned(), value.into());
    }

    fn fill(&self, argument: &str) -> Result<String> {
        let mut filled = String::with_capacity(argument.len());
        let mut rest = argument;
        while let Some(start) = rest.find("${") {
            filled.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find('}') else {
                filled.push_str(&rest[start..]);
                return Ok(filled);
            };
            let name = &after[..end];
            match self.0.get(name) {
                Some(value) => filled.push_str(value),
                None => {
                    return Err(Error::Placeholder {
                        argument: argument.to_owned(),
                        placeholder: name.to_owned(),
                    });
                }
            }
            rest = &after[end + 1..];
        }
        filled.push_str(rest);
        Ok(filled)
    }
}

pub fn expand(
    arguments: &[Argument],
    environment: &Environment,
    substitutions: &Substitutions,
) -> Result<Vec<String>> {
    let mut expanded = Vec::with_capacity(arguments.len());
    for argument in arguments {
        match argument {
            Argument::Plain(value) => expanded.push(substitutions.fill(value)?),
            Argument::Conditional { rules, value } if allowed(rules, environment) => match value {
                Values::One(value) => expanded.push(substitutions.fill(value)?),
                Values::Many(values) => {
                    for value in values {
                        expanded.push(substitutions.fill(value)?);
                    }
                }
            },
            Argument::Conditional { .. } => {}
        }
    }
    Ok(expanded)
}
