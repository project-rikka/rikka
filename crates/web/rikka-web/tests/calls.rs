use rikka_gecko_protocol::message::Release;
use rikka_gecko_protocol::{GeckoMessage, Target};
use rikka_web::{Error, SessionSettings, World};
use serde_json::{Value, json};

use crate::fake::FakeGecko;
use crate::support::block_on;

enum Answer {
    Success(Value),
    Failure(Value),
    Release,
}

#[test]
fn engine_replies_resolve_calls() {
    let cases = [
        (Answer::Success(json!(3)), Ok(json!(3))),
        (Answer::Failure(json!("boom")), Err("boom")),
        (
            Answer::Failure(json!({"message": "bad world"})),
            Err("bad world"),
        ),
        (Answer::Release, Err("unanswered")),
    ];
    let (runtime, mut gecko) = FakeGecko::started();
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let window = gecko.opened();
    gecko.ready(&window);

    for (answer, expected) in cases {
        let evaluating = std::thread::spawn({
            let session = session.clone();
            move || block_on(session.evaluate("1 + 2", &World::isolated("rikka-test")))
        });
        let event = gecko.next_event("Rikka:UserContent:Evaluate");
        assert_eq!(event.target, Target::Window(window.clone()));
        assert_eq!(
            event.data.as_ref().expect("a script")["world"],
            json!("rikka-test")
        );
        let callback = event.callback.expect("evaluation waits for an answer");
        match answer {
            Answer::Success(value) => gecko.reply(callback, true, value),
            Answer::Failure(value) => gecko.reply(callback, false, value),
            Answer::Release => gecko.send(&GeckoMessage::Release(Release { callback })),
        }
        let outcome = evaluating.join().expect("the evaluating thread");
        match (outcome, expected) {
            (Ok(value), Ok(expected)) => assert_eq!(value, expected),
            (Err(Error::Rejected(reason)), Err(expected)) => assert_eq!(reason, expected),
            (Err(Error::NoReply), Err("unanswered")) => {}
            (outcome, expected) => panic!("{outcome:?} is not {expected:?}"),
        }
    }
}
