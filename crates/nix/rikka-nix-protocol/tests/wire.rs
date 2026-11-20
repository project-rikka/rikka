use std::time::{Duration, UNIX_EPOCH};

use rikka_nix_log::{Count, Tally};
use rikka_nix_protocol::{
    Action, ChangeState, Error, Operation, Phase, Run, RunId, RunRecord, RunState, Status,
};

fn run(operation: Operation, state: RunState, finished: bool) -> Run {
    Run {
        id: RunId::new(1_762_400_000_000),
        operation,
        started: UNIX_EPOCH + Duration::from_millis(1_762_400_000_000),
        finished: finished.then(|| UNIX_EPOCH + Duration::from_millis(1_762_400_090_500)),
        state,
        status: Status {
            phase: Phase::Building,
            tally: Tally {
                builds: Count {
                    done: 3,
                    expected: 12,
                    running: 2,
                    failed: 0,
                },
                downloaded: Count {
                    done: 4096,
                    expected: 8192,
                    running: 0,
                    failed: 0,
                },
                ..Tally::default()
            },
        },
    }
}

#[test]
fn runs_survive_the_wire() {
    let runs = [
        run(
            Operation::Apply {
                action: Action::Switch,
            },
            RunState::Running,
            false,
        ),
        run(
            Operation::Update {
                action: Action::Boot,
            },
            RunState::Done {
                generation: Some(42),
            },
            true,
        ),
        run(
            Operation::Update {
                action: Action::Switch,
            },
            RunState::Done { generation: None },
            true,
        ),
        run(
            Operation::Rollback { generation: 7 },
            RunState::Failed {
                reason: "switch-to-configuration failed".to_owned(),
                changes: ChangeState::Modified,
            },
            true,
        ),
        run(
            Operation::Apply {
                action: Action::Test,
            },
            RunState::Failed {
                reason: "error: attribute missing".to_owned(),
                changes: ChangeState::Untouched,
            },
            true,
        ),
        run(
            Operation::Apply {
                action: Action::Build,
            },
            RunState::Interrupted,
            true,
        ),
    ];
    for original in runs {
        let record = RunRecord::from(&original);
        assert_eq!(Run::try_from(record).ok(), Some(original));
    }
}

#[test]
fn unknown_names_are_refused() {
    let base = RunRecord::from(&run(
        Operation::Apply {
            action: Action::Switch,
        },
        RunState::Running,
        false,
    ));
    let broken = [
        RunRecord {
            operation: "upgrade".to_owned(),
            ..base.clone()
        },
        RunRecord {
            action: "reboot".to_owned(),
            ..base.clone()
        },
        RunRecord {
            state: "paused".to_owned(),
            ..base.clone()
        },
    ];
    for record in broken {
        assert!(matches!(Run::try_from(record), Err(Error::Malformed(_))));
    }
}
