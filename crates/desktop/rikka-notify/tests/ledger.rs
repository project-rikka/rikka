use rikka_notify::{CloseReason, Event, Id, Invoked, Ledger};

fn id(raw: u32) -> Id {
    Id::new(raw).expect("a notification id")
}

fn invoked(raw: u32, key: &str) -> Event {
    Event::ActionInvoked {
        id: id(raw),
        key: key.to_owned(),
    }
}

#[test]
fn an_invoked_action_gives_what_was_shown_with_its_activation_token_once() {
    let mut ledger = Ledger::new();
    ledger.record(id(1), "first");
    ledger.record(id(2), "second");

    let token = Event::ActivationToken {
        id: id(2),
        token: "token-2".to_owned(),
    };
    assert_eq!(ledger.settle(token), None);
    assert_eq!(
        ledger.settle(invoked(2, "default")),
        Some(Invoked {
            shown: "second",
            key: "default".to_owned(),
            token: Some("token-2".to_owned()),
        })
    );
    assert_eq!(ledger.settle(invoked(2, "default")), None);
    assert_eq!(
        ledger.settle(invoked(1, "reply")),
        Some(Invoked {
            shown: "first",
            key: "reply".to_owned(),
            token: None,
        })
    );
    assert_eq!(ledger.settle(invoked(9, "default")), None);
}

#[test]
fn closed_and_removed_notifications_are_forgotten() {
    let mut ledger = Ledger::new();
    ledger.record(id(1), "first");
    ledger.record(id(2), "second");
    let closed = Event::Closed {
        id: id(1),
        reason: CloseReason::Dismissed,
    };
    assert_eq!(ledger.settle(closed), None);
    assert_eq!(ledger.settle(invoked(1, "default")), None);

    assert_eq!(ledger.find(|shown| *shown == "second"), Some(id(2)));
    assert_eq!(ledger.remove(id(2)), Some("second"));
    assert_eq!(ledger.find(|shown| *shown == "second"), None);
    assert_eq!(ledger.settle(invoked(2, "default")), None);
}
