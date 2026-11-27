use std::sync::Arc;

use rikka_agent::{Function, Reply};
use rikka_llm::{Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::file_access::FileAccess;

#[derive(Debug, Deserialize)]
struct Args {
    path: String,
    content: String,
}

pub(crate) fn declare(
    access: &Arc<FileAccess>,
    prompt: &Prompt,
    facts: &Facts,
) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("path", prompt.param("path", facts)?)
        .string("content", prompt.param("content", facts)?)
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
                .map_err(|error| format!("the file was not written: {error}"))?
        }
    }))
}

fn run(access: &FileAccess, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    let outcome = access
        .history
        .write(&access.files, &args.path, &args.content)
        .map_err(|error| error.to_string())?;

    let summary = match outcome.created {
        true => format!("Wrote {}, {} lines.", outcome.relative_path, outcome.lines),
        false => format!(
            "Rewrote {}, now {} lines.",
            outcome.relative_path, outcome.lines
        ),
    };
    Ok(Reply::new(
        match access.aside.instructions_for(&[outcome.path]) {
            Some(guides) => format!("{summary}\n\n{guides}"),
            None => summary,
        },
    ))
}
