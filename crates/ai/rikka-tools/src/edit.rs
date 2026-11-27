use std::sync::Arc;

use rikka_agent::{Function, Reply};
use rikka_file_edit::{Anchor, Edit, EditOutcome, EditRequest};
use rikka_llm::{Schema, Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::file_access::FileAccess;
use crate::path_instructions::PathInstructions;

const PATH: &str = "path";
const EDITS: &str = "edits";
const LINES: &str = "lines";
const AFTER: &str = "after";
const BEFORE: &str = "before";
const TEXT: &str = "text";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    path: String,
    edits: Vec<RawEdit>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEdit {
    lines: Option<Vec<String>>,
    after: Option<String>,
    before: Option<String>,
    text: String,
}

impl RawEdit {
    fn into_edit(self) -> std::result::Result<Edit, String> {
        let text = self.text;
        match (self.lines, self.after, self.before) {
            (Some(lines), None, None) => match lines.as_slice() {
                [only] => {
                    let line = anchor(only)?;
                    Ok(Edit::Replace {
                        first: line,
                        last: line,
                        text,
                    })
                }
                [first, last] => Ok(Edit::Replace {
                    first: anchor(first)?,
                    last: anchor(last)?,
                    text,
                }),
                _ => Err(format!(
                    "{LINES} takes one anchor, or the first and last of a range"
                )),
            },
            (None, Some(after), None) => Ok(Edit::InsertAfter {
                anchor: anchor(&after)?,
                text,
            }),
            (None, None, Some(before)) => Ok(Edit::InsertBefore {
                anchor: anchor(&before)?,
                text,
            }),
            _ => Err(format!("set exactly one of {LINES}, {AFTER} or {BEFORE}")),
        }
    }
}

pub(crate) fn declare(
    access: &Arc<FileAccess>,
    prompt: &Prompt,
    facts: &Facts,
) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string(PATH, prompt.param(PATH, facts)?)
        .param(EDITS, edits(prompt, facts)?)
        .build()
        .map_err(|error| Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;

    let access = Arc::clone(access);
    Ok(Function::new(tool, move |input: Value| {
        let access = Arc::clone(&access);
        async move {
            tokio::task::spawn_blocking(move || run(&access, input))
                .await
                .map_err(|error| format!("nothing was edited: {error}"))?
        }
    }))
}

fn edits(prompt: &Prompt, facts: &Facts) -> Result<Schema> {
    let item = Schema::object()
        .optional(
            LINES,
            Schema::array(Schema::string()).describe(prompt.param(LINES, facts)?),
        )
        .optional(
            AFTER,
            Schema::string().describe(prompt.param(AFTER, facts)?),
        )
        .optional(
            BEFORE,
            Schema::string().describe(prompt.param(BEFORE, facts)?),
        )
        .param(TEXT, Schema::string().describe(prompt.param(TEXT, facts)?))
        .closed();
    Ok(Schema::array(item).describe(prompt.param(EDITS, facts)?))
}

fn run(access: &FileAccess, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;
    let edits = args
        .edits
        .into_iter()
        .enumerate()
        .map(|(index, edit)| {
            edit.into_edit()
                .map_err(|error| format!("invalid edit {}: {error}", index + 1))
        })
        .collect::<std::result::Result<Vec<Edit>, String>>()?;

    let outcome = access
        .history
        .edit(
            &access.files,
            &EditRequest {
                path: &args.path,
                edits,
            },
        )
        .map_err(|error| error.to_string())?;

    Ok(edit_reply(access.aside.as_ref(), outcome))
}

fn anchor(text: &str) -> std::result::Result<Anchor, String> {
    text.parse()
        .map_err(|error: rikka_file_edit::InvalidAnchor| error.to_string())
}

fn edit_reply(aside: &dyn PathInstructions, outcome: EditOutcome) -> Reply {
    let mut summary = outcome.summary;
    if !outcome.excerpt.is_empty() {
        summary = format!("{summary}\n{}", outcome.excerpt);
    }
    if let Some(guides) = aside.instructions_for(&[outcome.path]) {
        summary = format!("{summary}\n\n{guides}");
    }

    let reply = Reply::new(summary);
    match serde_json::to_value(&outcome.change) {
        Ok(change) => reply.detail(change),
        Err(_) => reply,
    }
}
