use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rikka_compositor_input::{
    DeviceAccess, InputEvent, InputNotice, InputQueue, InputThread, OFlags, PointerShared,
};
use rikka_compositor_reactor::{Interest, Reactor};

#[derive(Debug, Default)]
struct Ledger {
    requests: Vec<(PathBuf, OFlags)>,
    open_count: usize,
    closed: usize,
}

#[derive(Debug, Default)]
struct Received {
    notices: Vec<InputNotice>,
    events: Vec<InputEvent>,
}

#[test]
fn seat_opens_and_closes_devices() {
    let ledger = Arc::new(Mutex::new(Ledger::default()));
    let access = {
        let opening = ledger.clone();
        let closing = ledger.clone();
        DeviceAccess::new(
            move |path, flags| {
                let mut ledger = opening.lock().unwrap();
                ledger.requests.push((path.to_owned(), flags));
                let fd = rustix::fs::open(path, flags, rustix::fs::Mode::empty())?;
                ledger.open_count += 1;
                Ok(fd)
            },
            move |fd| {
                drop(fd);
                closing.lock().unwrap().closed += 1;
            },
        )
    };

    let reactor = Reactor::<Received>::new().unwrap();
    let notices = reactor
        .mailbox(|received: &mut Received, notice| received.notices.push(notice))
        .unwrap();
    let queue = InputQueue::new().unwrap();
    let arrived = queue.clone();
    reactor
        .watch(
            queue.fd(),
            Interest::READABLE,
            move |received: &mut Received, _| arrived.drain(&mut received.events),
        )
        .unwrap();
    let input = InputThread::builder(&queue, notices, Arc::new(PointerShared::default()), access)
        .seat("seat0")
        .spawn()
        .unwrap();

    input.suspend().unwrap();
    input.resume().unwrap();
    drop(input);
    let mut received = Received::default();
    reactor.turn(&mut received, Some(Duration::ZERO)).unwrap();

    let ledger = ledger.lock().unwrap();
    assert!(
        !ledger.requests.is_empty(),
        "this machine has input devices"
    );
    for (path, flags) in &ledger.requests {
        assert!(path.starts_with("/dev/input"), "{}", path.display());
        assert!(flags.contains(OFlags::NONBLOCK), "{flags:?}");
    }
    assert_eq!(ledger.closed, ledger.open_count);

    assert!(
        !received
            .notices
            .iter()
            .any(|notice| matches!(notice, InputNotice::Failed(_)))
    );
    let added = received
        .events
        .iter()
        .filter(|event| matches!(event, InputEvent::DeviceAdded(_)))
        .count();
    assert!(added <= ledger.open_count);
}
