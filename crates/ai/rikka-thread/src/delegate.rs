use std::fmt;

use rikka_agent::{Function, Reply};
use rikka_llm::{Effort, Schema, Tool, Value};
use rikka_prompt::{Facts, Prompt, escape_attribute};
use serde::Deserialize;

use crate::error::{Error, Result, setup_error};
use crate::subagents::{DispatchReport, Subagents, TaskRequest};

#[derive(Debug, Deserialize)]
struct Args {
    tasks: Vec<AgentRequest>,
}

#[derive(Debug, Deserialize)]
struct AgentRequest {
    #[serde(default)]
    label: String,
    task: String,
    #[serde(default)]
    job: Option<u64>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
}

pub(crate) fn declare(subagents: &Subagents, prompt: &Prompt, facts: &Facts) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts).map_err(setup_error)?)
        .param("tasks", tasks(prompt, facts)?)
        .build()
        .map_err(|error| undeclarable(prompt.name(), &error))?;

    let most = usize::try_from(prompt.limit("most").map_err(setup_error)?).unwrap_or(usize::MAX);
    let subagents = subagents.clone();

    Ok(Function::new(tool, move |input: Value| {
        let subagents = subagents.clone();
        async move { run(&subagents, most, input) }
    }))
}

fn tasks(prompt: &Prompt, facts: &Facts) -> Result<Schema> {
    let described = |name: &str| prompt.param(name, facts).map_err(setup_error);
    let task_request = Schema::object()
        .param("task", Schema::string().describe(described("task")?))
        .optional("label", Schema::string().describe(described("label")?))
        .optional("job", Schema::integer().describe(described("job")?))
        .optional("model", Schema::string().describe(described("model")?))
        .optional(
            "effort",
            Schema::string()
                .enumerate(Effort::ALL.map(Effort::id))
                .describe(described("effort")?),
        );

    Ok(Schema::array(task_request).describe(described("tasks")?))
}

fn run(subagents: &Subagents, most: usize, input: Value) -> std::result::Result<Reply, String> {
    let args: Args =
        serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

    let task_requests = task_requests(args.tasks, most)?;
    let report = subagents.dispatch(task_requests)?;

    Ok(Reply::new(dispatch_report(&report)))
}

fn task_requests(
    tasks: Vec<AgentRequest>,
    most: usize,
) -> std::result::Result<Vec<TaskRequest>, String> {
    if tasks.is_empty() {
        return Err("the call names no agent to run".to_owned());
    }
    if tasks.len() > most {
        return Err(format!(
            "the call names {} agents, and {most} is as many as one call runs. Send the \
             work that matters most in one call of {most} or fewer.",
            tasks.len()
        ));
    }

    tasks
        .into_iter()
        .enumerate()
        .map(|(nth, request)| task_request(nth + 1, request))
        .collect()
}

fn task_request(nth: usize, request: AgentRequest) -> std::result::Result<TaskRequest, String> {
    let label = request.label.trim().to_owned();
    if request.task.trim().is_empty() {
        return Err(format!("agent {nth} was given no task at all"));
    }
    if request.job.is_none() && label.is_empty() {
        return Err(format!(
            "agent {nth} was given no label, and every agent started needs one"
        ));
    }

    let model = request
        .model
        .map(|named| named.trim().to_owned())
        .filter(|named| !named.is_empty());

    Ok(TaskRequest {
        label,
        task: request.task,
        job: request.job,
        model,
        effort: level(nth, request.effort.as_deref())?,
    })
}

fn level(nth: usize, effort: Option<&str>) -> std::result::Result<Option<Effort>, String> {
    let Some(named) = effort.map(str::trim).filter(|named| !named.is_empty()) else {
        return Ok(None);
    };

    Effort::from_id(&named.to_ascii_lowercase())
        .map(Some)
        .ok_or_else(|| {
            format!(
                "agent {nth} was given \"{named}\" to think at, and the levels are {}",
                Effort::ALL.map(Effort::id).join(", ")
            )
        })
}

fn dispatch_report(report: &DispatchReport) -> String {
    fmt::from_fn(|out| {
        out.write_str("<agents_running>")?;
        for agent in &report.started {
            write!(out, "\n<agent job=\"{}\"", agent.job)?;
            if !agent.label.is_empty() {
                write!(out, " label=\"{}\"", escape_attribute(&agent.label))?;
            }
            write!(
                out,
                " model=\"{}\" doing=\"{}\" />",
                escape_attribute(&agent.model),
                agent.kind.as_str()
            )?;
        }
        for trouble in &report.errors {
            write!(out, "\n<trouble>{}</trouble>", escape_attribute(trouble))?;
        }
        out.write_str(
            "\n</agents_running>\nEach report arrives as a notice when that agent finishes. Carry on \
             with your own work, or finish your turn — you will be brought back.",
        )
    })
    .to_string()
}

fn undeclarable(name: &str, error: &impl std::fmt::Display) -> Error {
    setup_error(format!("failed to declare \"{name}\": {error}"))
}
