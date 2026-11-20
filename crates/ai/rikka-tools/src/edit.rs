use std::sync::Arc;

use rikka_agent::{Function, Reply};
use rikka_file_edit::{EditOutcome, EditRequest, Files, Op};
use rikka_llm::{Schema, Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::file_history::FileHistory;
use crate::path_instructions::PathInstructions;

const REPLACE: &str = "replace";
const INSERT_BEFORE: &str = "insert_before";
const DELETE: &str = "delete";

#[derive(Debug, Deserialize)]
struct Args {
    path: String,
    ops: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum RawOp {
    Replace {
        start_line: usize,
        end_line: usize,
        text: String,
    },
    InsertBefore {
        line: usize,
        text: String,
    },
    Delete {
        start_line: usize,
        end_line: usize,
    },
}

impl From<RawOp> for Op {
    fn from(value: RawOp) -> Op {
        match value {
            RawOp::Replace {
                start_line,
                end_line,
                text,
            } => Op::Replace {
                start: start_line,
                end: end_line,
                text,
            },
            RawOp::InsertBefore { line, text } => Op::InsertBefore { line, text },
            RawOp::Delete {
                start_line,
                end_line,
            } => Op::Delete {
                start: start_line,
                end: end_line,
            },
        }
    }
}

pub(crate) fn declare(
    files: &Arc<Files>,
    history: &FileHistory,
    aside: &Arc<dyn PathInstructions>,
    prompt: &Prompt,
    facts: &Facts,
) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("path", prompt.param("path", facts)?)
        .param("ops", ops(prompt, facts)?)
        .build()
        .map_err(|error| Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;

    let files = Arc::clone(files);
    let history = history.clone();
    let aside = Arc::clone(aside);
    Ok(Function::new(tool, move |input: Value| {
        let files = Arc::clone(&files);
        let history = history.clone();
        let aside = Arc::clone(&aside);
        async move {
            tokio::task::spawn_blocking(move || run(&files, &history, aside.as_ref(), input))
                .await
                .map_err(|error| format!("nothing was edited: {error}"))?
        }
    }))
}

fn ops(prompt: &Prompt, facts: &Facts) -> Result<Schema> {
    Ok(Schema::array(Schema::any_of([
        covering(prompt, facts, REPLACE)?
            .param(
                "text",
                described(prompt, facts, "replace_text", Schema::string())?,
            )
            .closed(),
        Schema::object()
            .param("op", named(prompt, facts, INSERT_BEFORE)?)
            .param("line", described(prompt, facts, "line", Schema::integer())?)
            .param(
                "text",
                described(prompt, facts, "insert_text", Schema::string())?,
            )
            .closed(),
        covering(prompt, facts, DELETE)?.closed(),
    ]))
    .describe(prompt.param("ops", facts)?))
}

fn covering(prompt: &Prompt, facts: &Facts, op: &str) -> Result<Schema> {
    Ok(Schema::object()
        .param("op", named(prompt, facts, op)?)
        .param(
            "start_line",
            described(prompt, facts, "start_line", Schema::integer())?,
        )
        .param(
            "end_line",
            described(prompt, facts, "end_line", Schema::integer())?,
        ))
}

fn named(prompt: &Prompt, facts: &Facts, op: &str) -> Result<Schema> {
    described(prompt, facts, op, Schema::string().enumerate([op]))
}

fn described(prompt: &Prompt, facts: &Facts, name: &str, schema: Schema) -> Result<Schema> {
    Ok(schema.describe(prompt.param(name, facts)?))
}

fn run(
    files: &Files,
    history: &FileHistory,
    aside: &dyn PathInstructions,
    input: Value,
) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    let ops = args
        .ops
        .into_iter()
        .enumerate()
        .map(|(at, value)| parse_op(at + 1, value))
        .collect::<std::result::Result<Vec<Op>, String>>()?;

    let outcome = history
        .edit(
            files,
            &EditRequest {
                path: &args.path,
                ops,
            },
        )
        .map_err(|error| error.to_string())?;

    Ok(edit_reply(aside, outcome))
}

fn parse_op(at: usize, value: Value) -> std::result::Result<Op, String> {
    serde_json::from_value::<RawOp>(value)
        .map(Op::from)
        .map_err(|error| format!("invalid op{at}: {error}"))
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
