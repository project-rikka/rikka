use rikka_format::format_duration;
use rikka_nix::{Action, ChangeState, Operation, Phase, Run, RunState, Status, Tally};
use rikka_notifications::Progress;
use rikka_ui::tr;
use shell_kit::launch::{self, Spawn};
use shell_notifications::{ActionRequest, Announcement};

const ICON: &str = "system-software-update";

const SEPARATOR: &str = " · ";

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shown {
    summary: String,
    body: String,
    progress: Option<Progress>,
}

impl From<&Run> for Shown {
    fn from(run: &Run) -> Shown {
        match &run.state {
            RunState::Running => Shown {
                summary: running(run.operation),
                body: phase(run.operation, run.status),
                progress: Some(gauge(run.status)),
            },
            RunState::Done { generation } => Shown {
                summary: done(run.operation),
                body: finished(run, *generation),
                progress: None,
            },
            RunState::Failed { reason, changes } => Shown {
                summary: failed(run.operation),
                body: [reason.clone(), consequence(*changes)].join("\n"),
                progress: None,
            },
            RunState::Interrupted => Shown {
                summary: failed(run.operation),
                body: [
                    tr!("shell.system-update.interrupted").to_string(),
                    consequence(ChangeState::Modified),
                ]
                .join("\n"),
                progress: None,
            },
        }
    }
}

pub(super) fn announcement(run: &Run, shown: &Shown) -> Announcement {
    let open = ActionRequest::new(launch::CAPABILITY, launch::SPAWN).encode(&Spawn::new([
        launch::UPDATES.to_owned(),
        "--run".to_owned(),
        run.id.to_string(),
    ]));
    let announcement = Announcement::new(
        tr!("shell.system-update.app-name").to_string(),
        shown.summary.clone(),
    )
    .body(shown.body.clone())
    .image(ICON)
    .tag(format!("system-update-{}", run.id))
    .action_request(open.clone())
    .button(tr!("shell.system-update.show-log").to_string(), open);
    let announcement = match shown.progress {
        Some(progress) => announcement.resident().progress(progress),
        None => announcement,
    };
    match run.state {
        RunState::Failed { .. } | RunState::Interrupted => announcement.urgent(),
        RunState::Running | RunState::Done { .. } => announcement,
    }
}

fn running(operation: Operation) -> String {
    match operation {
        Operation::Apply { .. } => tr!("shell.system-update.applying"),
        Operation::Update { .. } => tr!("shell.system-update.updating"),
        Operation::Rollback { generation } => {
            tr!("shell.system-update.rolling-back", generation = generation)
        }
    }
    .to_string()
}

fn done(operation: Operation) -> String {
    match operation {
        Operation::Apply { .. } => tr!("shell.system-update.applied"),
        Operation::Update { .. } => tr!("shell.system-update.updated"),
        Operation::Rollback { generation } => {
            tr!("shell.system-update.rolled-back", generation = generation)
        }
    }
    .to_string()
}

fn failed(operation: Operation) -> String {
    match operation {
        Operation::Apply { .. } => tr!("shell.system-update.not-applied"),
        Operation::Update { .. } => tr!("shell.system-update.not-updated"),
        Operation::Rollback { generation } => {
            tr!(
                "shell.system-update.not-rolled-back",
                generation = generation
            )
        }
    }
    .to_string()
}

fn phase(operation: Operation, status: Status) -> String {
    match status.phase {
        Phase::Preparing => tr!("shell.system-update.preparing").to_string(),
        Phase::Locking => tr!("shell.system-update.locking").to_string(),
        Phase::Evaluating => tr!("shell.system-update.evaluating").to_string(),
        Phase::Building => {
            let counted = counts(status.tally);
            match counted.is_empty() {
                true => tr!("shell.system-update.building").to_string(),
                false => counted,
            }
        }
        Phase::Activating => match operation.action() {
            Some(Action::Boot) => tr!("shell.system-update.staging"),
            Some(Action::Build) => tr!("shell.system-update.checking"),
            Some(Action::Switch | Action::Test) | None => tr!("shell.system-update.activating"),
        }
        .to_string(),
    }
}

fn counts(tally: Tally) -> String {
    let mut parts = Vec::new();
    if tally.builds.expected > 0 {
        parts.push(
            tr!(
                "shell.system-update.built",
                done = tally.builds.done,
                expected = tally.builds.expected
            )
            .to_string(),
        );
    }
    if tally.paths.expected > 0 {
        parts.push(
            tr!(
                "shell.system-update.fetched",
                done = tally.paths.done,
                expected = tally.paths.expected
            )
            .to_string(),
        );
    }
    parts.join(SEPARATOR)
}

fn gauge(status: Status) -> Progress {
    status
        .fraction()
        .and_then(|fraction| Progress::percent((fraction * 100.0).round() as i32))
        .unwrap_or(Progress::Indeterminate)
}

fn finished(run: &Run, generation: Option<u32>) -> String {
    let mut parts = Vec::new();
    if let Some(generation) = generation {
        parts.push(tr!("shell.system-update.generation", generation = generation).to_string());
    }
    if let Some(took) = run
        .finished
        .and_then(|finished| finished.duration_since(run.started).ok())
    {
        parts.push(format_duration(took));
    }
    if run.operation.action() == Some(Action::Boot) {
        parts.push(tr!("shell.system-update.next-boot").to_string());
    }
    parts.join(SEPARATOR)
}

fn consequence(changes: ChangeState) -> String {
    match changes {
        ChangeState::Untouched => tr!("shell.system-update.untouched"),
        ChangeState::Modified => tr!("shell.system-update.modified"),
    }
    .to_string()
}
