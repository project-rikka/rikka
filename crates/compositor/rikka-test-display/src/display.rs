use std::cell::Cell;
use std::fmt;
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rikka_compositor_reactor::{Reactor, Sender};
use rikka_compositor_server::ClientOptions;

use crate::host::Host;
use crate::notes::Notes;

pub const PATIENCE: Duration = Duration::from_secs(5);

const THREAD_NAME: &str = "rikka-test-display";

enum Job<S> {
    Run(Box<dyn FnOnce(&mut S) + Send>),
    Stop,
}

pub struct Display<S: Host> {
    jobs: Sender<Job<S>>,
    thread: Option<JoinHandle<()>>,
    notes: Receiver<String>,
}

impl<S: Host> Display<S> {
    pub fn start(build: impl FnOnce(&Reactor<S>, Notes) -> S + Send + 'static) -> Self {
        let (noting, notes) = Notes::channel();
        let (ready, jobs) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name(THREAD_NAME.to_owned())
            .spawn(move || {
                let reactor = Reactor::<S>::new().expect("a reactor for the test display");
                let mut state = build(&reactor, noting.clone());
                state.display_handle().on_disconnect(move |_, _, reason| {
                    noting.note(format!("disconnected: {reason}"));
                });
                let stopped = Rc::new(Cell::new(false));
                let stopping = Rc::clone(&stopped);
                let jobs = reactor
                    .mailbox(move |state: &mut S, job: Job<S>| match job {
                        Job::Run(task) => task(state),
                        Job::Stop => stopping.set(true),
                    })
                    .expect("a mailbox for the test display");
                if ready.send(jobs).is_err() {
                    return;
                }
                reactor
                    .run(&mut state, |_| stopped.get())
                    .expect("the test display's reactor runs");
            })
            .expect("a thread for the test display");
        let jobs = jobs.recv().expect("the test display starts");
        Self {
            jobs,
            thread: Some(thread),
            notes,
        }
    }

    pub fn call<T: Send + 'static>(&self, task: impl FnOnce(&mut S) -> T + Send + 'static) -> T {
        let (reply, answer) = mpsc::channel();
        let sent = self.jobs.send(Job::Run(Box::new(move |state| {
            let _ = reply.send(task(state));
        })));
        assert!(sent.is_ok(), "the test display stopped");
        answer
            .recv_timeout(PATIENCE)
            .expect("the test display answers")
    }

    pub fn connect(&self, options: ClientOptions) -> UnixStream {
        let (ours, theirs) = UnixStream::pair().expect("a socket pair");
        self.call(move |state| {
            state
                .display_handle()
                .insert_client(theirs, options)
                .expect("the test display admits the client");
        });
        ours
    }

    pub fn expect(&self, fragment: &str) {
        let deadline = Instant::now() + PATIENCE;
        let mut received = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.notes.recv_timeout(left) {
                Ok(line) if line.contains(fragment) => return,
                Ok(line) => received.push(line),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    panic!("never saw {fragment:?}; saw {received:?}")
                }
            }
        }
    }
}

impl<S: Host> Drop for Display<S> {
    fn drop(&mut self) {
        let _ = self.jobs.send(Job::Stop);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
            && !std::thread::panicking()
        {
            panic!("the display thread panicked");
        }
    }
}

impl<S: Host> fmt::Debug for Display<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Display")
            .field("running", &self.thread.is_some())
            .finish_non_exhaustive()
    }
}
