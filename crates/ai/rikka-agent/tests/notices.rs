use std::sync::Mutex;

use rikka_agent::{Agent, Event, Notice, Notices};
use rikka_test_llm::Scripted;

#[tokio::test]
async fn waiting_run_stays_open_for_pending_work() {
    let script = Scripted::new();
    script.says("It is running.").says("It finished cleanly.");

    let notices = Notices::new();
    let pending = Mutex::new(Some(notices.pending()));

    let agent = Agent::new(script.model())
        .notices(notices.clone())
        .wait_for_notices(true)
        .observer(move |event| {
            if matches!(event, Event::TurnFinished { turn: 1, .. })
                && let Some(pending) = pending.lock().unwrap().take()
            {
                pending.post(Notice::new("<job_ended id=\"1\" state=\"exited 0\" />"));
            }
        })
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent.run("start it").await.expect("run should finish");

    assert_eq!(outcome.text(), "It finished cleanly.");
    assert_eq!(outcome.turns(), 2, "the notice starts another turn");
    assert_eq!(notices.outstanding(), 0);

    let requests = script.requests();
    let messages = requests
        .last()
        .expect("second turn should exist")
        .messages();
    let notice = messages
        .iter()
        .find(|message| message.text().contains("job_ended"))
        .expect("notice should reach the model");
    assert_eq!(notice.role(), "user");
}

#[tokio::test]
async fn non_waiting_run_ends_with_pending_work() {
    let script = Scripted::new();
    script.says("Started; I will not wait.");

    let notices = Notices::new();
    let pending = notices.pending();

    let agent = Agent::new(script.model())
        .notices(notices.clone())
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent.run("start it").await.expect("run should finish");

    assert_eq!(outcome.turns(), 1);
    assert_eq!(notices.outstanding(), 1, "the work outlives the run");
    drop(pending);
}

#[tokio::test]
async fn silent_work_does_not_hang_run() {
    let script = Scripted::new();
    script.says("Started.");

    let notices = Notices::new();
    let pending = Mutex::new(Some(notices.pending()));

    let agent = Agent::new(script.model())
        .notices(notices.clone())
        .wait_for_notices(true)
        .observer(move |event| {
            if matches!(event, Event::TurnFinished { turn: 1, .. }) {
                drop(pending.lock().unwrap().take());
            }
        })
        .build()
        .expect("agent configuration should be valid");

    let outcome = agent.run("start it").await.expect("run should finish");

    assert_eq!(outcome.text(), "Started.");
    assert_eq!(outcome.turns(), 1);
}

#[test]
fn mailbox_idle_requires_no_pending_or_unread() {
    let notices = Notices::new();
    assert!(notices.is_quiet());

    let pending = notices.pending();
    assert!(!notices.is_quiet(), "work is still expected");

    pending.post(Notice::new("<job_ended id=\"1\" state=\"exited 0\" />"));
    assert!(!notices.is_quiet(), "what it said is still unread");

    assert_eq!(notices.take().len(), 1);
    assert!(notices.is_quiet());
}

#[test]
fn held_notices_wait_for_the_next_turn() {
    let notices = Notices::new();
    notices.post(Notice::new("<job_ended id=\"1\" state=\"stopped by the person\" />").held());
    assert!(!notices.is_empty());
    assert!(!notices.is_waking(), "a held notice starts no turn");

    notices.post(Notice::new("<job_ended id=\"2\" state=\"reported\" />"));
    assert!(notices.is_waking());

    notices.hold();
    assert!(
        !notices.is_waking(),
        "what was posted before a hold stays held"
    );
    assert_eq!(notices.take().len(), 2, "held notices are still delivered");
}
