use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use rikka_seam::{
    Call, Dispatcher, EventInfo, Manifest, Method, MethodInfo, Payload, Request, Responder,
    SEAM_CAPABILITY, Seam, SeamError, Service, ServiceContext, Version,
};

type Queue = Arc<Mutex<VecDeque<Call>>>;

struct QueueDispatcher(Queue);

impl Dispatcher for QueueDispatcher {
    fn enqueue(&self, call: Call) {
        self.0
            .lock()
            .expect("queue mutex should not be poisoned")
            .push_back(call);
    }
}

struct Host {
    seam: Seam,
    queue: Queue,
}

impl Host {
    fn new() -> Self {
        let queue = Queue::default();
        Self {
            seam: Seam::new(QueueDispatcher(Arc::clone(&queue))),
            queue,
        }
    }

    fn run(&mut self) {
        loop {
            let next = self
                .queue
                .lock()
                .expect("queue mutex should not be poisoned")
                .pop_front();
            let Some(call) = next else { return };
            self.seam.dispatch(&mut (), call);
        }
    }

    async fn call(&mut self, request: Request) -> Result<Payload, SeamError> {
        let reply = self.seam.client().call(request);
        self.run();
        reply.await
    }
}

#[derive(Default)]
struct Counter {
    seam: Option<ServiceContext>,
    total: i64,
}

impl Service for Counter {
    fn manifest(&self) -> Manifest {
        Manifest::new("test.counter", Version::new(1, 2))
            .summary("A number that goes up")
            .method(MethodInfo::new("add"))
            .method(MethodInfo::new("total"))
            .event(EventInfo::new("changed"))
    }

    fn attach(&mut self, seam: ServiceContext) {
        self.seam = Some(seam);
    }

    fn call(&mut self, _: &mut (), method: Method, call: Call) {
        match &*method {
            "add" => {
                let decoded = call.decode::<i64>();
                let amount = match decoded {
                    Ok(amount) => amount,
                    Err(error) => return call.fail(error),
                };
                self.total += amount;
                if let Some(seam) = &self.seam {
                    seam.publisher()
                        .emit("changed", &self.total)
                        .expect("`changed` event should be declared");
                }
                call.encode(&self.total);
            }
            "total" => call.encode(&self.total),
            _ => call.unknown_method(),
        }
    }
}

struct Slow(Arc<Mutex<Option<Responder>>>);

impl Service for Slow {
    fn manifest(&self) -> Manifest {
        Manifest::new("test.slow", Version::new(1, 0)).method(MethodInfo::new("answer"))
    }

    fn call(&mut self, _: &mut (), _method: Method, call: Call) {
        let (_request, responder) = call.defer();
        *self.0.lock().expect("slot mutex should not be poisoned") = Some(responder);
    }
}

struct Forgetful;

impl Service for Forgetful {
    fn manifest(&self) -> Manifest {
        Manifest::new("test.forgetful", Version::new(1, 0)).method(MethodInfo::new("ignore"))
    }

    fn call(&mut self, _: &mut (), _method: Method, call: Call) {
        drop(call);
    }
}

#[test]
fn capability_is_provided_once() {
    let mut host = Host::new();
    host.seam.register(Counter::default()).unwrap();
    assert_eq!(
        host.seam.register(Counter::default()).unwrap_err(),
        SeamError::DuplicateCapability {
            capability: "test.counter".into(),
        }
    );
}

#[test]
fn seam_capability_is_reserved() {
    struct Impostor;
    impl Service for Impostor {
        fn manifest(&self) -> Manifest {
            Manifest::new(SEAM_CAPABILITY, Version::new(9, 0))
        }
        fn call(&mut self, _: &mut (), _method: Method, call: Call) {
            call.done();
        }
    }

    let mut host = Host::new();
    assert!(matches!(
        host.seam.register(Impostor).unwrap_err(),
        SeamError::DuplicateCapability { .. }
    ));
}

#[test]
fn unroutable_manifest_is_rejected() {
    struct Unroutable;
    impl Service for Unroutable {
        fn manifest(&self) -> Manifest {
            Manifest::new("Test/Counter", Version::new(1, 0))
        }
        fn call(&mut self, _: &mut (), _method: Method, call: Call) {
            call.done();
        }
    }

    let mut host = Host::new();
    assert_eq!(
        host.seam.register(Unroutable).unwrap_err(),
        SeamError::InvalidName {
            name: "Test/Counter".into(),
        }
    );
}

#[tokio::test]
async fn request_reaches_service_and_replies() {
    let mut host = Host::new();
    host.seam.register(Counter::default()).unwrap();

    let total = host
        .call(Request::new("test.counter", "add").encode(&7i64))
        .await
        .unwrap();
    assert_eq!(total.decode::<i64>().unwrap(), 7);

    let total = host
        .call(Request::new("test.counter", "total"))
        .await
        .unwrap();
    assert_eq!(total.decode::<i64>().unwrap(), 7);
}

#[tokio::test]
async fn unknown_capability_gets_error_reply() {
    let mut host = Host::new();
    assert_eq!(
        host.call(Request::new("test.counter", "add"))
            .await
            .unwrap_err(),
        SeamError::UnknownCapability {
            capability: "test.counter".into(),
        }
    );
}

#[tokio::test]
async fn undeclared_method_is_rejected() {
    let mut host = Host::new();
    host.seam.register(Counter::default()).unwrap();
    assert_eq!(
        host.call(Request::new("test.counter", "subtract"))
            .await
            .unwrap_err(),
        SeamError::unknown_method("test.counter", "subtract")
    );
}

#[tokio::test]
async fn unsupported_version_is_rejected() {
    let mut host = Host::new();
    host.seam.register(Counter::default()).unwrap();

    let request = Request::new("test.counter", "total").require(Version::new(1, 1));
    assert!(host.call(request).await.is_ok());

    let request = Request::new("test.counter", "total").require(Version::new(1, 9));
    assert_eq!(
        host.call(request).await.unwrap_err(),
        SeamError::VersionMismatch {
            capability: "test.counter".into(),
            required: Version::new(1, 9),
            provided: Version::new(1, 2),
        }
    );
}

#[tokio::test]
async fn deferred_call_replies_later() {
    let pending = Arc::new(Mutex::new(None));
    let mut host = Host::new();
    host.seam.register(Slow(Arc::clone(&pending))).unwrap();

    let reply = host.seam.client().call(Request::new("test.slow", "answer"));
    host.run();

    let responder = pending
        .lock()
        .unwrap()
        .take()
        .expect("service should defer the call");
    responder.encode(&"eventually");

    assert_eq!(
        reply.await.unwrap().decode::<String>().unwrap(),
        "eventually"
    );
}

#[tokio::test]
async fn dropped_call_replies_no_reply() {
    let mut host = Host::new();
    host.seam.register(Forgetful).unwrap();
    assert_eq!(
        host.call(Request::new("test.forgetful", "ignore"))
            .await
            .unwrap_err(),
        SeamError::NoReply {
            capability: "test.forgetful".into(),
            method: "ignore".into(),
        }
    );
}

#[tokio::test]
async fn published_event_reaches_subscribers() {
    let mut host = Host::new();
    host.seam.register(Counter::default()).unwrap();

    let mut events = host.seam.client().subscribe_to("test.counter");
    host.call(Request::new("test.counter", "add").encode(&2i64))
        .await
        .unwrap();

    let delivery = events.next().await.unwrap();
    let event = delivery.event().expect("delivery should be an event");
    assert_eq!(event.capability().as_str(), "test.counter");
    assert_eq!(event.name(), "changed");
    assert_eq!(event.decode::<i64>().unwrap(), 2);
}

#[tokio::test]
async fn seam_lists_all_capabilities() {
    let mut host = Host::new();
    host.seam.register(Counter::default()).unwrap();

    let payload = host
        .call(Request::new(SEAM_CAPABILITY, "capabilities"))
        .await
        .unwrap();
    let manifests: Vec<Manifest> = payload.decode().unwrap();

    let names: Vec<&str> = manifests
        .iter()
        .map(|manifest| manifest.capability().as_str())
        .collect();
    assert_eq!(names, [SEAM_CAPABILITY, "test.counter"]);

    let counter = &manifests[1];
    assert_eq!(counter.version(), Version::new(1, 2));
    assert_eq!(counter.summary_text(), Some("A number that goes up"));
    assert!(counter.declares_method("add"));
    assert!(counter.declares_event("changed"));
}

struct Ledger {
    seam: Seam<Ledger>,
    queue: Queue,
    balance: i64,
}

struct Deposit;

impl Service<Ledger> for Deposit {
    fn manifest(&self) -> Manifest {
        Manifest::new("test.ledger", Version::new(1, 0)).method(MethodInfo::new("deposit"))
    }

    fn call(&mut self, ledger: &mut Ledger, _: Method, call: Call) {
        match call.decode::<i64>() {
            Ok(amount) => {
                ledger.balance += amount;
                ledger
                    .seam
                    .client()
                    .notify(Request::new(SEAM_CAPABILITY, "capabilities"));
                call.encode(&ledger.balance);
            }
            Err(error) => call.fail(error),
        }
    }
}

#[tokio::test]
async fn service_reaches_the_host_that_owns_its_seam() {
    let queue = Queue::default();
    let mut ledger = Ledger {
        seam: Seam::new(QueueDispatcher(Arc::clone(&queue))),
        queue,
        balance: 10,
    };
    ledger.seam.register(Deposit).unwrap();

    let reply = ledger
        .seam
        .client()
        .call(Request::new("test.ledger", "deposit").encode(&5i64));
    loop {
        let next = ledger
            .queue
            .lock()
            .expect("queue mutex should not be poisoned")
            .pop_front();
        let Some(call) = next else { break };
        Seam::dispatch_within(&mut ledger, call, |ledger| &mut ledger.seam);
    }

    assert_eq!(reply.await.unwrap().decode::<i64>().unwrap(), 15);
    assert_eq!(ledger.balance, 15);
}
