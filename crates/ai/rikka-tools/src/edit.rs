use std::sync::Arc;

use rikka_agent::{Function, Reply};
use rikka_file_edit::{EditOutcome, EditRequest, Replacement};
use rikka_llm::{Schema, Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::file_access::FileAccess;
use crate::path_instructions::PathInstructions;

const OLD: &str = "old_string";
const NEW: &str = "new_string";
const ALL: &str = "replace_all";
const EDITS: &str = "edits";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    path: String,
    old_string: Option<String>,
    new_string: Option<String>,
    replace_all: Option<bool>,
    edits: Option<Vec<RawEdit>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEdit {
    old_string: String,
    new_string: String,
    replace_all: Option<bool>,
}

impl From<RawEdit> for Replacement {
    fn from(edit: RawEdit) -> Replacement {
        replacement(edit.old_string, edit.new_string, edit.replace_all)
    }
}

pub(crate) fn declare(
    access: &Arc<FileAccess>,
    prompt: &Prompt,
    facts: &Facts,
) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("path", prompt.param("path", facts)?)
        .string(OLD, prompt.param(OLD, facts)?)
        .string(NEW, prompt.param(NEW, facts)?)
        .boolean(ALL, prompt.param(ALL, facts)?)
        .param(EDITS, edits(prompt, facts)?)
        .optional(OLD)
        .optional(NEW)
        .optional(ALL)
        .optional(EDITS)
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
        .param(OLD, Schema::string().describe(prompt.param(OLD, facts)?))
        .param(NEW, Schema::string().describe(prompt.param(NEW, facts)?))
        .optional(ALL, Schema::boolean().describe(prompt.param(ALL, facts)?))
        .closed();
    Ok(Schema::array(item).describe(prompt.param(EDITS, facts)?))
}

fn run(access: &FileAccess, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;
    let path = args.path.clone();
    let replacements = replacements(args)?;

    let outcome = access
        .history
        .edit(
            &access.files,
            &EditRequest {
                path: &path,
                replacements,
            },
        )
        .map_err(|error| error.to_string())?;

    Ok(edit_reply(access.aside.as_ref(), outcome))
}

fn replacements(args: Args) -> std::result::Result<Vec<Replacement>, String> {
    match (args.old_string, args.new_string, args.edits) {
        (Some(old), Some(new), None) => Ok(vec![replacement(old, new, args.replace_all)]),
        (None, None, Some(edits)) if args.replace_all.is_none() => {
            Ok(edits.into_iter().map(Replacement::from).collect())
        }
        (None, None, Some(_)) => Err(format!("set {ALL} inside each of {EDITS}")),
        (_, _, Some(_)) => Err(format!("set either {OLD} and {NEW}, or {EDITS}, not both")),
        (Some(_), None, None) => Err(format!("{NEW} is missing")),
        (None, Some(_), None) => Err(format!("{OLD} is missing")),
        (None, None, None) => Err(format!("set {OLD} and {NEW}, or {EDITS}")),
    }
}

fn replacement(old: String, new: String, all: Option<bool>) -> Replacement {
    match all.unwrap_or(false) {
        true => Replacement::all(old, new),
        false => Replacement::new(old, new),
    }
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
