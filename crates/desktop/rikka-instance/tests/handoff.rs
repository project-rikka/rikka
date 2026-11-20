use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use rikka_instance::{LaunchRequest, Startup, claim};
use rikka_test_bus::{PrivateBus, TIMEOUT};

const ID: &str = "rikka-instance-test";

#[test]
fn second_start_forwards_to_primary() {
    let Some(_bus) = PrivateBus::start() else {
        return;
    };

    let (forwarded, received) = mpsc::channel();
    let sink = Arc::new(move |open: LaunchRequest| {
        forwarded
            .send(open)
            .expect("test receives forwarded requests");
    });

    let Startup::Primary(service) = claim(ID, sink) else {
        panic!("the name is free on a bus of our own");
    };
    assert_eq!(service.name().bus(), "org.rikka.InstanceTest");
    assert_eq!(service.name().path(), "/org/rikka/InstanceTest");

    let Startup::Running(peer) = claim(ID, Arc::new(|_| {})) else {
        panic!("the name is held by the one above");
    };

    peer.forward(
        &LaunchRequest::from_uris(vec!["file:///etc/hostname".to_owned()])
            .with_token("a-token")
            .with_cwd("/tmp"),
    )
    .expect("the one already running takes it");

    let request = next_request(&received);
    assert_eq!(request.uris(), ["file:///etc/hostname"]);
    assert_eq!(request.paths(), [PathBuf::from("/etc/hostname")]);
    assert_eq!(
        request.token(),
        Some("a-token"),
        "the token is what the window is shown on the strength of"
    );
    assert_eq!(
        request.cwd(),
        Some(Path::new("/tmp")),
        "where the start happened outlives the process it happened in"
    );

    peer.forward(&LaunchRequest::default())
        .expect("a start naming nothing is still a start");

    let bare = next_request(&received);
    assert!(bare.is_bare());
    assert_eq!(bare.token(), None);

    let started = [
        "rikka-instance-test",
        "--new-window",
        "https://example.com",
        "notes.txt",
    ];
    peer.forward(
        &LaunchRequest::from_arguments(started.map(OsString::from))
            .with_token("another-token")
            .with_cwd("/srv"),
    )
    .expect("a start with options is handed over too");

    let commanded = next_request(&received);
    assert_eq!(
        commanded.arguments(),
        started[1..].iter().map(OsString::from).collect::<Vec<_>>(),
        "the arguments arrive as they were given"
    );
    assert!(commanded.has_flag("--new-window"));
    assert_eq!(
        commanded.uris(),
        ["https://example.com", "file:///srv/notes.txt"],
        "a relative path is read from where the second start happened"
    );
    assert_eq!(commanded.token(), Some("another-token"));
}

fn next_request(received: &Receiver<LaunchRequest>) -> LaunchRequest {
    received
        .recv_timeout(TIMEOUT)
        .expect("primary instance receives the request")
}
