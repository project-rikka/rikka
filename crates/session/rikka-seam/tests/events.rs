use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rikka_seam::{
    Call, CapabilityId, Dispatcher, EventInfo, EventQueue, Manifest, Method, Publisher, Seam,
    Service, ServiceContext, Version,
};

const FLOOD: u64 = 4096;

struct Dropping;

impl Dispatcher for Dropping {
    fn enqueue(&self, call: Call) {
        call.unknown_method();
    }
}

struct Announcer {
    manifest: Manifest,
    context: Arc<Mutex<Option<ServiceContext>>>,
}

impl Service for Announcer {
    fn manifest(&self) -> Manifest {
        self.manifest.clone()
    }

    fn attach(&mut self, seam: ServiceContext) {
        *self.context.lock().expect("mutex should not be poisoned") = Some(seam);
    }

    fn call(&mut self, _: &mut (), _method: Method, call: Call) {
        call.unknown_method();
    }
}

fn announcing(seam: &mut Seam, capability: &str, event: EventInfo) -> Publisher {
    let context = Arc::new(Mutex::new(None));
    seam.register(Announcer {
        manifest: Manifest::new(capability, Version::new(1, 0)).event(event),
        context: Arc::clone(&context),
    })
    .expect("announcer should register");
    let state = context.lock().expect("mutex should not be poisoned");
    state
        .as_ref()
        .expect("service should have a context after registration")
        .publisher()
        .clone()
}

fn drained(queue: &mut EventQueue) -> (u64, Vec<u64>) {
    let mut lagged = 0;
    let mut received = Vec::new();
    while let Some(next) = queue.try_next() {
        if let Some(gap) = next.lagged() {
            lagged += gap.count();
        }
        if let Some(event) = next.event() {
            received.push(
                event
                    .decode::<u64>()
                    .expect("event payload should decode as u64"),
            );
        }
    }
    (lagged, received)
}

#[test]
fn lagging_subscriber_receives_lag_count() {
    let mut seam = Seam::new(Dropping);
    let publisher = announcing(&mut seam, "test.flood", EventInfo::new("changed"));
    let mut queue = seam.client().poll_events(None::<&str>);

    for at in 0..FLOOD {
        publisher
            .emit("changed", &at)
            .expect("`changed` event should be declared");
    }

    let (lagged, received) = drained(&mut queue);
    assert!(lagged > 0, "the queue is bounded");
    assert_eq!(
        lagged + received.len() as u64,
        FLOOD,
        "every event is accounted for"
    );
    assert_eq!(received.last(), Some(&(FLOOD - 1)));
}

#[test]
fn latest_state_event_replaces_pending() {
    let mut seam = Seam::new(Dropping);
    let publisher = announcing(&mut seam, "test.state", EventInfo::new("changed").latest());
    let mut queue = seam.client().poll_events(None::<&str>);

    for at in 0..FLOOD {
        publisher
            .emit("changed", &at)
            .expect("`changed` event should be declared");
    }

    assert_eq!(drained(&mut queue), (0, vec![FLOOD - 1]));
}

#[test]
fn burst_does_not_affect_other_subscriber() {
    let mut seam = Seam::new(Dropping);
    let noisy = announcing(&mut seam, "test.noise", EventInfo::new("changed"));
    let quiet = announcing(&mut seam, "test.quiet", EventInfo::new("changed"));
    let mut queue = seam
        .client()
        .poll_events(Some(CapabilityId::from("test.quiet")));

    quiet
        .emit("changed", &7u64)
        .expect("`changed` event should be declared");
    for at in 0..FLOOD {
        noisy
            .emit("changed", &at)
            .expect("`changed` event should be declared");
    }

    assert_eq!(drained(&mut queue), (0, vec![7]));
    assert!(
        noisy.emit("cleared", &0u64).is_err(),
        "an undeclared event is refused"
    );
}

#[test]
fn next_timeout_returns_event_or_times_out() {
    let mut seam = Seam::new(Dropping);
    let publisher = announcing(&mut seam, "test.wait", EventInfo::new("changed"));
    let mut queue = seam.client().poll_events(None::<&str>);

    assert_eq!(
        queue.next_timeout(Duration::from_millis(10)),
        Err(RecvTimeoutError::Timeout)
    );

    let emitting = thread::spawn(move || {
        publisher
            .emit("changed", &3u64)
            .expect("`changed` event should be declared");
    });
    let delivery = queue
        .next_timeout(Duration::from_secs(10))
        .expect("event should arrive before the timeout");
    emitting.join().expect("emitting thread should not panic");
    let event = delivery.event().expect("delivery should be an event");
    assert_eq!(event.decode::<u64>().ok(), Some(3));
}
