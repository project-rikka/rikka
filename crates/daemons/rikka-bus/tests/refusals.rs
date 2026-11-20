use rikka_bus::{Refused, Refusing};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Connect,
    Scan,
}

#[derive(Default)]
struct State {
    error: Option<Refused<Action>>,
}

impl Refusing for State {
    type Action = Action;

    fn refused(&mut self) -> &mut Option<Refused<Action>> {
        &mut self.error
    }
}

fn refused(action: Action) -> Option<Refused<Action>> {
    Some(Refused {
        action,
        reason: "denied".to_owned(),
    })
}

#[test]
fn answers_record_failures_and_clear_only_their_own_refusal() {
    let cases = [
        (
            None,
            Action::Connect,
            Some("denied"),
            refused(Action::Connect),
            true,
        ),
        (refused(Action::Connect), Action::Connect, None, None, true),
        (
            refused(Action::Connect),
            Action::Scan,
            None,
            refused(Action::Connect),
            false,
        ),
        (
            refused(Action::Connect),
            Action::Scan,
            Some("denied"),
            refused(Action::Scan),
            true,
        ),
        (
            refused(Action::Connect),
            Action::Connect,
            Some("denied"),
            refused(Action::Connect),
            false,
        ),
        (None, Action::Scan, None, None, false),
    ];
    for (held, action, error, expected, moved) in cases {
        let mut state = State {
            error: held.clone(),
        };
        let answered = state.answer(action, error.map(str::to_owned));
        assert_eq!(
            state.error, expected,
            "{held:?} answered {action:?} with {error:?}"
        );
        assert_eq!(
            answered, moved,
            "{held:?} answered {action:?} with {error:?}"
        );
    }
}
