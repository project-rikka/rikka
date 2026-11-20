use std::os::fd::AsFd as _;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use rikka_worker::{Reply, SendError, Stop as _, command_channel};

const TICK: Duration = Duration::from_millis(50);

enum Request {
    Double(u32, Reply<u32>),
    Stop,
}

#[test]
fn request_returns_worker_reply() {
    let (sender, receiver) = command_channel(|| Request::Stop).expect("a command channel");
    let worker = std::thread::spawn(move || {
        let (idle, _other) = UnixStream::pair().expect("a socket pair");
        loop {
            for request in receiver.drain() {
                match request {
                    Request::Double(number, reply) => drop(reply.send(number * 2)),
                    Request::Stop => return,
                }
            }
            receiver.wait_readable(idle.as_fd(), TICK).expect("a wait");
        }
    });

    let response = sender.request(|reply| Request::Double(21, reply), TICK * 20);
    sender.stop();
    worker.join().expect("a joined worker");

    assert_eq!(response.expect("a response"), 42);
}

#[test]
fn request_distinguishes_timeout_from_disconnect() {
    let (sender, receiver) = command_channel(|| Request::Stop).expect("a command channel");

    let unanswered = sender.request(|reply| Request::Double(1, reply), TICK);
    assert!(matches!(unanswered, Err(SendError::Timeout)));

    drop(receiver);
    let disconnected = sender.request(|reply| Request::Double(1, reply), TICK);
    assert!(matches!(disconnected, Err(SendError::Disconnected)));
}
