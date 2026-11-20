use std::time::SystemTime;

use rikka_app::prelude::*;
use rikka_format::{format_bytes, format_duration};
use rikka_nix::{Action, ChangeState, Operation, Phase, Run, RunState, Status};

const SEPARATOR: &str = " · ";

pub(crate) fn title(run: &Run) -> Label {
    match run.operation {
        Operation::Apply { .. } => tr!("updates.operation.apply"),
        Operation::Update { .. } => tr!("updates.operation.update"),
        Operation::Rollback { generation } => {
            tr!("updates.operation.rollback", generation = generation)
        }
    }
}

pub(crate) fn symbol(run: &Run) -> (MaterialSymbol, Color) {
    let palette = palette();
    match run.state {
        RunState::Running => (MaterialSymbol::SYNC, palette.accent),
        RunState::Done { .. } => (MaterialSymbol::CHECK_CIRCLE, palette.success),
        RunState::Failed { .. } => (MaterialSymbol::ERROR, palette.danger),
        RunState::Interrupted => (MaterialSymbol::WARNING, palette.warning),
    }
}

pub(crate) fn state(run: &Run) -> Label {
    match run.state {
        RunState::Running => tr!("updates.state.running"),
        RunState::Done { .. } => tr!("updates.state.done"),
        RunState::Failed { .. } => tr!("updates.state.failed"),
        RunState::Interrupted => tr!("updates.state.interrupted"),
    }
}

pub(crate) fn when(run: &Run) -> String {
    stamp(run.started)
}

pub(crate) fn summary(run: &Run) -> String {
    let mut parts = vec![tr!("updates.detail.started", time = &stamp(run.started)).to_string()];
    if let RunState::Done {
        generation: Some(generation),
    } = run.state
    {
        parts.push(tr!("updates.detail.generation", generation = generation).to_string());
    }
    let took = run
        .finished
        .and_then(|finished| finished.duration_since(run.started).ok());
    if let Some(took) = took {
        parts.push(tr!("updates.detail.took", duration = &format_duration(took)).to_string());
    }
    if matches!(run.state, RunState::Done { .. }) && run.operation.action() == Some(Action::Boot) {
        parts.push(tr!("updates.detail.next-boot").to_string());
    }
    parts.join(SEPARATOR)
}

pub(crate) fn phase(run: &Run) -> Label {
    match run.status.phase {
        Phase::Preparing => tr!("updates.phase.preparing"),
        Phase::Locking => tr!("updates.phase.locking"),
        Phase::Evaluating => tr!("updates.phase.evaluating"),
        Phase::Building => tr!("updates.phase.building"),
        Phase::Activating => match run.operation.action() {
            Some(Action::Boot) => tr!("updates.phase.staging"),
            Some(Action::Build) => tr!("updates.phase.checking"),
            Some(Action::Switch | Action::Test) | None => tr!("updates.phase.activating"),
        },
    }
}

pub(crate) fn work(status: Status) -> String {
    let tally = status.tally;
    let mut parts = Vec::new();
    if tally.builds.expected > 0 {
        parts.push(
            tr!(
                "updates.detail.built",
                done = tally.builds.done,
                expected = tally.builds.expected
            )
            .to_string(),
        );
    }
    if tally.paths.expected > 0 {
        parts.push(
            tr!(
                "updates.detail.fetched",
                done = tally.paths.done,
                expected = tally.paths.expected
            )
            .to_string(),
        );
    }
    if tally.downloaded.done > 0 {
        parts.push(
            tr!(
                "updates.detail.downloaded",
                size = &format_bytes(tally.downloaded.done)
            )
            .to_string(),
        );
    }
    parts.join(SEPARATOR)
}

pub(crate) fn failure(run: &Run) -> Option<(String, Label)> {
    match &run.state {
        RunState::Failed { reason, changes } => Some((reason.clone(), consequence(*changes))),
        RunState::Interrupted => Some((
            tr!("updates.detail.interrupted").to_string(),
            consequence(ChangeState::Modified),
        )),
        RunState::Running | RunState::Done { .. } => None,
    }
}

pub(crate) fn file_name(run: &Run) -> String {
    let at = jiff::Zoned::try_from(run.started)
        .map(|zoned| zoned.strftime("%Y%m%d-%H%M%S").to_string())
        .unwrap_or_else(|_| run.id.to_string());
    format!("rikka-{}-{at}.log", run.operation.name())
}

fn consequence(changes: ChangeState) -> Label {
    match changes {
        ChangeState::Untouched => tr!("updates.detail.untouched"),
        ChangeState::Modified => tr!("updates.detail.modified"),
    }
}

fn stamp(time: SystemTime) -> String {
    let Ok(zoned) = jiff::Zoned::try_from(time) else {
        return String::new();
    };
    let today = jiff::Zoned::now().date();
    match zoned.date() == today {
        true => zoned.strftime("%H:%M").to_string(),
        false => zoned.strftime("%Y-%m-%d %H:%M").to_string(),
    }
}
