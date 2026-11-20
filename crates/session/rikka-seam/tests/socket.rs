use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use rikka_seam::net::{SocketClient, SocketListener};
use rikka_seam::{
    Call, EventInfo, Manifest, Method, MethodInfo, Request, SEAM_CAPABILITY, Service,
    ServiceContext, Version,
};

struct Counter {
    seam: Option<ServiceContext>,
    total: i64,
}

impl Service for Counter {
    fn manifest(&self) -> Manifest {
        Manifest::new("example.counter", Version::new(1, 2))
            .summary("Counts")
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
                let by = match call.decode::<i64>() {
                    Ok(by) => by,
                    Err(error) => return call.fail(error),
                };
                self.total += by;
                if let Some(seam) = &self.seam {
                    let _ = seam.publisher().emit("changed", &self.total);
                }
                call.encode(&self.total);
            }
            "total" => call.encode(&self.total),
            _ => call.unknown_method(),
        }
    }
}

#[derive(Default)]
struct Sink {
    calls: Vec<Call>,
}

impl Service for Sink {
    fn manifest(&self) -> Manifest {
        Manifest::new("example.sink", Version::new(1, 0)).method(MethodInfo::new("hold"))
    }

    fn call(&mut self, _: &mut (), method: Method, call: Call) {
        match &*method {
            "hold" => self.calls.push(call),
            _ => call.unknown_method(),
        }
    }
}

struct Host {
    _listener: SocketListener,
    path: PathBuf,
    _seam: rikka_test_seam::Host,
}

impl Host {
    fn start(directory: &tempfile::TempDir) -> Self {
        let path = directory.path().join("seam.sock");
        let seam = rikka_test_seam::Host::start(|seam| {
            seam.register(Counter {
                seam: None,
                total: 0,
            })
            .expect("counter should register");
            seam.register(Sink::default())
                .expect("sink should register");
        });
        let listener = SocketListener::serve(&path, seam.client().clone())
            .expect("socket should bind in the temp dir");

        Self {
            _listener: listener,
            path,
            _seam: seam,
        }
    }
}

fn connect(host: &Host) -> SocketClient {
    SocketClient::connect(&host.path).expect("host should be listening")
}

#[tokio::test]
async fn request_round_trips_over_socket() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::start(&directory);
    let socket = connect(&host);
    let seam = socket.client();

    let reply = seam
        .call(rikka_seam::Request::new("example.counter", "add").encode(&7i64))
        .await
        .expect("counter should reply");
    assert_eq!(reply.decode::<i64>().unwrap(), 7);

    let total = seam
        .call(rikka_seam::Request::new("example.counter", "total"))
        .await
        .expect("counter should reply");
    assert_eq!(total.decode::<i64>().unwrap(), 7);
}

#[tokio::test]
async fn remote_event_is_delivered() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::start(&directory);
    let socket = connect(&host);
    let seam = socket.client();
    let mut events = seam.subscribe();

    seam.notify(rikka_seam::Request::new("example.counter", "add").encode(&3i64));

    let delivery = events.next().await.expect("counter should announce");
    let event = delivery.event().expect("delivery should be an event");
    assert_eq!(event.capability().as_str(), "example.counter");
    assert_eq!(event.name(), "changed");
    assert_eq!(event.decode::<i64>().unwrap(), 3);
}

#[tokio::test]
async fn remote_seam_lists_manifests() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::start(&directory);
    let socket = connect(&host);

    let listed = socket
        .client()
        .call(rikka_seam::Request::new(SEAM_CAPABILITY, "capabilities"))
        .await
        .expect("seam should list its capabilities");
    let manifests: Vec<Manifest> = listed.decode().unwrap();
    assert!(
        manifests
            .iter()
            .any(|manifest| manifest.capability().as_str() == "example.counter")
    );
}

#[tokio::test]
async fn remote_error_keeps_code_and_message() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::start(&directory);
    let socket = connect(&host);
    let seam = socket.client();

    let missing = seam
        .call(rikka_seam::Request::new("example.nothing", "add"))
        .await
        .unwrap_err();
    assert_eq!(missing.code(), 100, "routing band");
    assert!(missing.to_string().contains("example.nothing"));

    let too_new = seam
        .call(rikka_seam::Request::new("example.counter", "add").require(Version::new(2, 0)))
        .await
        .unwrap_err();
    assert_eq!(too_new.code(), 103);

    let wrong_shape = seam
        .call(rikka_seam::Request::new("example.counter", "add").encode("seven"))
        .await
        .unwrap_err();
    assert_eq!(wrong_shape.code(), 300, "payload band");
}

#[tokio::test]
async fn calls_fail_after_disconnect() {
    let directory = tempfile::tempdir().unwrap();
    let socket = {
        let host = Host::start(&directory);
        connect(&host)
    };

    socket.closed().await;
    assert!(!socket.is_connected());
    let error = socket
        .client()
        .call(rikka_seam::Request::new("example.counter", "total"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), 500, "transport band");
}

#[test]
fn disconnect_closes_event_queues() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::start(&directory);
    let socket = connect(&host);
    let seam = socket.client();
    let mut events = seam.poll_events(None::<&str>);

    let (done, finished) = std::sync::mpsc::channel();
    let waiting = std::thread::spawn(move || {
        while events.blocking_next().is_some() {}
        let _ = done.send(());
    });

    drop(socket);

    finished
        .recv_timeout(Duration::from_secs(5))
        .expect("queue should end with its connection");
    waiting.join().expect("relay thread should end");
}

#[tokio::test]
async fn flooding_peer_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::start(&directory);
    let socket = connect(&host);
    let seam = socket.client();

    let mut calls: FuturesUnordered<_> = (0..1024)
        .map(|_| Box::pin(seam.call(Request::new("example.sink", "hold"))))
        .collect();

    let first = tokio::time::timeout(Duration::from_secs(10), calls.next())
        .await
        .expect("flooding peer should be refused");

    assert!(
        matches!(&first, Some(Err(error)) if error.code() == 501),
        "asking past the connection's limit is answered with the overload code, not {first:?}"
    );
}

#[tokio::test]
async fn oversized_frame_closes_only_that_peer() {
    let directory = tempfile::tempdir().unwrap();
    let host = Host::start(&directory);

    let mut stranger = UnixStream::connect(&host.path).expect("host should be listening");
    stranger
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("socket timeout should be set");
    stranger
        .write_all(&u32::MAX.to_le_bytes())
        .expect("header should be sent");
    let _ = stranger.write_all(b"not read");
    let mut answer = [0u8; 64];
    let read = stranger.read(&mut answer);
    assert!(
        match &read {
            Ok(count) => *count == 0,
            Err(error) => !matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ),
        },
        "the host hangs up rather than answering a frame it never read: {read:?}"
    );

    let total = connect(&host)
        .client()
        .call(Request::new("example.counter", "total"))
        .await
        .expect("counter should still reply");
    assert_eq!(total.decode::<i64>().unwrap(), 0);
}
