use rikka_nix::{Action, Error, Outcome, Progress, Rebuild, Refusal, Request, Stage};

fn requested() -> Rebuild {
    let mut rebuild = Rebuild::new();
    drop(rebuild.start(Request::Update {
        action: Action::Switch,
    }));
    rebuild
}

#[test]
fn rebuild_runs_to_completion() {
    let mut rebuild = requested();
    assert_eq!(rebuild.stage(), Stage::Requested);

    assert_eq!(rebuild.handle_progress(Progress::Started), None);
    assert_eq!(rebuild.stage(), Stage::Running);

    let outcome = rebuild.handle_progress(Progress::Finished(Outcome::Done {
        generation: Some(42),
    }));
    assert_eq!(
        outcome,
        Some(Outcome::Done {
            generation: Some(42)
        })
    );
    assert_eq!(rebuild.stage(), Stage::Idle);
    assert_eq!(
        rebuild.handle_progress(Progress::Finished(Outcome::Done { generation: None })),
        None
    );
}

#[test]
fn request_that_never_started_is_refused() {
    let cases = [
        (Error::Denied, None),
        (
            Error::Unavailable("no bus".to_owned()),
            Some(Refusal::Unavailable("no bus".to_owned())),
        ),
    ];
    for (error, shown) in cases {
        let mut rebuild = requested();
        rebuild.fail(&error);
        assert_eq!(rebuild.stage(), Stage::Idle);
        assert_eq!(rebuild.refusal(), shown.as_ref());
    }
}
