use std::sync::Arc;

use rikka_agent::{Function, Reply};
use rikka_file_edit::Files;
use rikka_llm::{Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::file_history::FileHistory;
use crate::path_instructions::PathInstructions;

#[derive(Debug, Deserialize)]
struct Args {
    path: String,
    content: String,
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
        .string("content", prompt.param("content", facts)?)
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
                .map_err(|error| format!("the file was not written: {error}"))?
        }
    }))
}

fn run(
    files: &Files,
    history: &FileHistory,
    aside: &dyn PathInstructions,
    input: Value,
) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    let outcome = history
        .write(files, &args.path, &args.content)
        .map_err(|error| error.to_string())?;

    let summary = match outcome.created {
        true => format!("Wrote {}, {} lines.", outcome.relative_path, outcome.lines),
        false => format!(
            "Rewrote {}, now {} lines.",
            outcome.relative_path, outcome.lines
        ),
    };
    Ok(Reply::new(match aside.instructions_for(&[outcome.path]) {
        Some(guides) => format!("{summary}\n\n{guides}"),
        None => summary,
    }))
}
