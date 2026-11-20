use rikka_web::{CloseReason, Error, Event, Exit, Find, RuntimeEvent, SessionSettings};

use crate::fake::FakeGecko;
use crate::support::{block_on, next_matching};

#[test]
fn engine_crash_ends_sessions_allows_restart() {
    let (runtime, mut gecko) = FakeGecko::started();
    let mut runtime_events = runtime.events().expect("the first to ask takes the events");
    let session = runtime
        .session(SessionSettings::new())
        .expect("a session opens");
    let mut events = session.events().expect("the first to ask takes the events");
    let window = gecko.opened();
    gecko.ready(&window);

    let finding = std::thread::spawn({
        let session = session.clone();
        move || block_on(session.find(Find::text("rikka")))
    });
    gecko.next_event("GeckoView:FindInPage");
    gecko.crash();

    let exit = next_matching(&mut runtime_events, |event| match event {
        RuntimeEvent::Exited(exit) => Some(exit),
        _ => None,
    });
    assert_eq!(exit, Exit::Signal(9));
    let reason = next_matching(&mut events, |event| match event {
        Event::Closed(reason) => Some(reason),
        _ => None,
    });
    assert_eq!(reason, CloseReason::EngineExited);
    assert!(matches!(
        finding.join().expect("the find thread"),
        Err(Error::EngineExited)
    ));
    assert!(!runtime.is_running());
    assert!(matches!(
        runtime.session(SessionSettings::new()),
        Err(Error::EngineExited)
    ));

    let (again, mut restarted) = FakeGecko::started();
    let _reopened = again
        .session(SessionSettings::new())
        .expect("the new runtime opens sessions");
    restarted.opened();
}
