use std::collections::HashMap;

use serde::{Deserialize, Serialize};

const PROMPT_REFERENCE: &str = "ref/prompt";

#[derive(Debug, Serialize)]
pub(crate) struct CompleteParams {
    #[serde(rename = "ref")]
    reference: Reference,
    argument: Argument,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<Context>,
}

#[derive(Debug, Serialize)]
struct Reference {
    #[serde(rename = "type")]
    kind: &'static str,
    name: String,
}

#[derive(Debug, Serialize)]
struct Argument {
    name: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct Context {
    arguments: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompleteResult {
    pub(crate) completion: Completion,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Completion {
    #[serde(default)]
    pub(crate) values: Vec<String>,
}

impl CompleteParams {
    pub(crate) fn prompt_argument(
        prompt: &str,
        argument: &str,
        typed: &str,
        given: HashMap<String, String>,
    ) -> CompleteParams {
        CompleteParams {
            reference: Reference {
                kind: PROMPT_REFERENCE,
                name: prompt.to_owned(),
            },
            argument: Argument {
                name: argument.to_owned(),
                value: typed.to_owned(),
            },
            context: (!given.is_empty()).then_some(Context { arguments: given }),
        }
    }
}
