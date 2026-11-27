use std::future::Future;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use futures_channel::mpsc::UnboundedReceiver;
use futures_util::StreamExt;
use rikka_web::{Event, PageState, Runtime, RuntimeEvent, Session};
use rikka_worker::block_on_timeout;

const STARTUP: Duration = Duration::from_secs(45);

const PATIENCE: Duration = Duration::from_secs(30);

pub(crate) struct Engine {
    runtime: Option<Runtime>,
    pub(crate) events: UnboundedReceiver<RuntimeEvent>,
}

impl Engine {
    pub(crate) fn start() -> Engine {
        let runtime = Runtime::builder().start().expect("Chrome starts");
        let mut events = runtime.events().expect("the first to ask takes the events");
        next_matching_within(STARTUP, &mut events, |event| match event {
            RuntimeEvent::Ready { .. } => Some(()),
            RuntimeEvent::Failed(failure) => panic!("the extension failed: {failure:?}"),
            RuntimeEvent::Exited(exit) => panic!("Chrome exited: {exit:?}"),
            _ => None,
        });
        Engine {
            runtime: Some(runtime),
            events,
        }
    }
}

impl std::ops::Deref for Engine {
    type Target = Runtime;

    fn deref(&self) -> &Runtime {
        self.runtime
            .as_ref()
            .expect("the engine lives until dropped")
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.runtime.take();
        rikka_web::wait_for_closed_runtimes();
    }
}

pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    block_on_timeout(future, PATIENCE).expect("the future settles in time")
}

pub(crate) fn next_matching<T, R>(
    events: &mut UnboundedReceiver<T>,
    matched: impl FnMut(T) -> Option<R>,
) -> R {
    next_matching_within(PATIENCE, events, matched)
}

fn next_matching_within<T, R>(
    patience: Duration,
    events: &mut UnboundedReceiver<T>,
    mut matched: impl FnMut(T) -> Option<R>,
) -> R {
    let deadline = Instant::now() + patience;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = block_on_timeout(events.next(), left)
            .expect("the expected event arrives in time")
            .expect("the event stream stays open");
        if let Some(result) = matched(event) {
            return result;
        }
    }
}

pub(crate) fn until_page(
    session: &Session,
    events: &mut UnboundedReceiver<Event>,
    settled: impl Fn(&PageState) -> bool,
) {
    while !settled(&session.page()) {
        next_matching(events, |event| match event {
            Event::Closed(reason) => panic!("the session went away: {reason:?}"),
            _ => Some(()),
        });
    }
}

pub(crate) fn served(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let address = listener.local_addr().expect("a local address");
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                line.clear();
            }
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}/")
}
