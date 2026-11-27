use std::sync::Arc;

use rikka_agent::{Function, Reply};
use rikka_file_edit::Files;
use rikka_llm::{Tool, Value, json};
use rikka_prompt::{Facts, Prompt};
use serde::Deserialize;

use crate::error::Result;
use crate::jobs::Jobs;
use crate::known;
use crate::listing::listing;
use crate::logs::{Log, Logs};
use crate::look::look;
use crate::machine::Machine;
use crate::reach::Reach;
use crate::shell::{CommandSpec, Ended, KillOnDrop, Limits, Running};

#[derive(Debug, Deserialize)]
struct Args {
    command: String,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default)]
    background: bool,
}

pub(crate) struct Bash {
    pub(crate) files: Arc<Files>,
    pub(crate) jobs: Jobs,
    pub(crate) logs: Logs,
    pub(crate) machine: Option<Arc<dyn Machine>>,
    pub(crate) reach: Reach,
}

pub(crate) fn declare(bash: Bash, prompt: &Prompt, facts: &Facts) -> Result<Function> {
    let tool = Tool::new(prompt.name())
        .description(prompt.body(facts)?)
        .string("command", prompt.param("command", facts)?)
        .integer("timeout", prompt.param("timeout", facts)?)
        .boolean("background", prompt.param("background", facts)?)
        .optional("timeout")
        .optional("background")
        .build()
        .map_err(|error| crate::error::Error::Undeclarable {
            name: prompt.name().to_owned(),
            reason: error.to_string(),
        })?;

    let limits = Limits::read(prompt)?;
    let bash = Arc::new(bash);
    Ok(Function::new(tool, move |input: Value| {
        let bash = Arc::clone(&bash);
        async move { bash.run(limits, input).await }
    }))
}

impl Bash {
    async fn run(&self, limits: Limits, input: Value) -> std::result::Result<Reply, String> {
        let args: Args =
            serde_json::from_value(input).map_err(|error| format!("invalid arguments: {error}"))?;

        if self.reach == Reach::Reading && look(&args.command).is_none() {
            return Err(format!(
                "`{}` was not run: only commands that read, search or list run here",
                args.command
            ));
        }
        let room = limits.output();
        let reading = known::read(&args.command);
        let builds = reading.builds();
        let log = self.logs.open(reading.named());
        let running = Running::start(
            CommandSpec {
                command: &args.command,
                at: self.files.root(),
                machine: self.machine.as_deref(),
                log: log.clone(),
            },
            limits.retain(),
        )
        .inspect_err(|_| forget(&log))?;
        let mut kill_guard = KillOnDrop::new(&running);

        if args.background {
            kill_guard.disarm();
            let id = self
                .jobs
                .command(&args.command, running, log_notice(&log, builds));
            return Ok(Reply::new(started(id, log.as_ref())));
        }

        let timeout = limits.timeout(args.timeout);
        let Some(ended) = running.wait_timeout(timeout).await else {
            if waits_only(&args.command) {
                kill_guard.disarm();
                running.kill();
                running.wait_exit().await;
                forget(&log);
                let timeout = timeout.as_millis();
                return Ok(replied(
                    cut(
                        &format!("the command was still running after {timeout}ms and was killed"),
                        &running.output(room),
                    ),
                    true,
                    None,
                ));
            }

            kill_guard.disarm();
            let output = match builds {
                true => None,
                false => Some(running.read(room)),
            };
            let id = self
                .jobs
                .command(&args.command, running, log_notice(&log, builds));
            return Ok(Reply::new(handed(
                id,
                timeout.as_millis(),
                output.as_deref(),
                log.as_ref(),
            )));
        };

        kill_guard.disarm();

        let output = running.output(room);
        let output = match (self.reach, builds) {
            (Reach::Working, false) => self.anchored(&args.command, output).await?,
            _ => output,
        };
        Ok(command_reply(ended, &output, log, builds, room))
    }

    async fn anchored(&self, command: &str, output: String) -> std::result::Result<String, String> {
        let Some(listing) = listing(command) else {
            return Ok(output);
        };
        let listing = listing.except(self.logs.folder().display().to_string());
        let files = Arc::clone(&self.files);
        tokio::task::spawn_blocking(move || files.anchor(&listing, &output))
            .await
            .map_err(|error| format!("failed to anchor the output: {error}"))
    }
}

fn command_reply(ended: Ended, output: &str, log: Option<Log>, builds: bool, room: usize) -> Reply {
    let (status, failed) = ending(ended);
    let log = match log {
        Some(log) if builds || log.bytes() > room => Some(log),
        Some(log) => {
            log.forget();
            None
        }
        None => None,
    };

    let text = match (&log, builds) {
        (Some(log), true) => format!("{status}. {}", whole(log)),
        (Some(log), false) => format!("{}\n\n{}", plainly(&status, failed, output), whole(log)),
        (None, _) => plainly(&status, failed, output),
    };

    replied(text, failed, log.as_ref())
}

fn plainly(status: &str, failed: bool, output: &str) -> String {
    match (failed, output.is_empty()) {
        (true, _) => cut(status, output),
        (false, true) => "(it printed nothing)".to_owned(),
        (false, false) => output.to_owned(),
    }
}

fn ending(ended: Ended) -> (String, bool) {
    match ended {
        Ended::Exited(0) => ("the command succeeded".to_owned(), false),
        Ended::Exited(code) => (format!("the command exited with code {code}"), true),
        Ended::Signalled => ("the command was killed by a signal".to_owned(), true),
    }
}

fn replied(text: String, failed: bool, log: Option<&Log>) -> Reply {
    let reply = match failed {
        true => Reply::new(text).failed(),
        false => Reply::new(text),
    };

    match log {
        Some(log) => reply.detail(json!({ "at": log.path().display().to_string() })),
        None => reply,
    }
}

fn log_notice(log: &Option<Log>, builds: bool) -> Option<String> {
    log.as_ref()
        .filter(|_| builds)
        .map(|log| log.display_path().to_owned())
}

fn forget(log: &Option<Log>) {
    if let Some(log) = log {
        log.forget();
    }
}

fn whole(log: &Log) -> String {
    format!(
        "Its whole output is in {} ({} bytes); read what you need out of that file.",
        log.display_path(),
        log.bytes()
    )
}

fn started(id: u64, log: Option<&Log>) -> String {
    let started = format!(
        "it is running in the background as job {id}. You will be told when it ends; read what \
         it printed with the Jobs tool."
    );
    match log {
        Some(log) => format!(
            "{started} Everything it prints is written to {}.",
            log.display_path()
        ),
        None => started,
    }
}

fn handed(id: u64, elapsed_ms: u128, output: Option<&str>, log: Option<&Log>) -> String {
    let handed = format!(
        "the command was still running after {elapsed_ms}ms, so it was moved to the background as \
         job {id} rather than killed. You will be told when it ends."
    );
    let where_it_goes = match log {
        Some(log) => format!(
            " Everything it prints is written to {}.",
            log.display_path()
        ),
        None => String::new(),
    };

    match output.map(str::trim).filter(|output| !output.is_empty()) {
        Some(output) => format!("{handed}{where_it_goes} What it had printed by then:\n{output}"),
        None => format!("{handed}{where_it_goes}"),
    }
}

fn cut(reason: &str, output: &str) -> String {
    match output.is_empty() {
        true => reason.to_owned(),
        false => format!("{reason}.\n{output}"),
    }
}

fn waits_only(command: &str) -> bool {
    let command = command.trim();
    if command.contains([';', '&', '|', '\n']) {
        return false;
    }

    command
        .split_whitespace()
        .next()
        .and_then(|program| program.rsplit('/').next())
        .is_some_and(|program| program == "sleep")
}
