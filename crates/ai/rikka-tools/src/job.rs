use rikka_agent::{Function, Reply};
use rikka_llm::{Tool, Value};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::jobs::{Jobs, unknown};
use crate::limits::most;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Doing {
    List,
    Output,
    Stop,
}

#[derive(Debug, Deserialize)]
struct Args {
    action: Doing,
    #[serde(default)]
    id: Option<u64>,
}

pub(crate) fn declare(jobs: &Jobs, prompt: &Prompt, facts: &Facts) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("action", prompt.param("action", facts)?)
        .integer("id", prompt.param("id", facts)?)
        .optional("id")
        .build()
        .map_err(|error| Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;
    let room = most(prompt, "output_bytes")?;

    let jobs = jobs.clone();
    Ok(Function::new(tool, move |input: Value| {
        let jobs = jobs.clone();
        async move { run(&jobs, room, input) }
    }))
}

fn run(jobs: &Jobs, room: usize, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    match args.action {
        Doing::List => Ok(Reply::new(jobs.listing())),

        Doing::Output => {
            let id = args.id.ok_or_else(|| named("read"))?;
            Ok(Reply::new(jobs.read(id, room)?))
        }

        Doing::Stop => {
            let id = args.id.ok_or_else(|| named("stop"))?;
            let Some(label) = jobs.label(id) else {
                return Err(unknown(id));
            };
            match jobs.stop(id) {
                true => Ok(Reply::new(format!("job {id} was stopped: {label}"))),
                false => Ok(Reply::new(format!("job {id} had already ended: {label}"))),
            }
        }
    }
}

fn named(doing: &str) -> String {
    format!("the call says nothing about which job to {doing}")
}
