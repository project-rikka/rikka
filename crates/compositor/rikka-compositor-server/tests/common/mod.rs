mod handlers;
mod raw;

use std::any::Any;
use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rikka_compositor_reactor::{Reactor, Sender};
use rikka_compositor_server::{ClientOptions, DisplayHandle, Global};

pub(crate) use handlers::{Counter, Tag, serve};
pub(crate) use raw::RawClient;

pub(crate) const PATIENCE: Duration = Duration::from_secs(5);

type Task = Box<dyn FnOnce(&mut State) + Send>;

pub(crate) struct State {
    pub(crate) display: DisplayHandle<State>,
    pub(crate) globals: HashMap<&'static str, Global>,
    pub(crate) retained: Vec<Box<dyn Any>>,
    pub(crate) counted: u32,
    log: mpsc::Sender<String>,
    stop: bool,
}

impl State {
    pub(crate) fn note(&self, line: impl Into<String>) {
        let _ = self.log.send(line.into());
    }
}

pub(crate) struct Server {
    tasks: Option<Sender<Task>>,
    thread: Option<JoinHandle<()>>,
    log: Receiver<String>,
}

impl Server {
    pub(crate) fn start(setup: impl FnOnce(&mut State) + Send + 'static) -> Self {
        let (log, lines) = mpsc::channel();
        let (ready, tasks) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("rikka-test-display".into())
            .spawn(move || {
                let reactor = Reactor::<State>::new().unwrap();
                let display = DisplayHandle::new(&reactor);
                let mut state = State {
                    display,
                    globals: HashMap::new(),
                    retained: Vec::new(),
                    counted: 0,
                    log,
                    stop: false,
                };
                let sender = reactor
                    .mailbox(|state: &mut State, task: Task| task(state))
                    .unwrap();
                setup(&mut state);
                ready.send(sender).unwrap();
                reactor.run(&mut state, |state| state.stop).unwrap();
            })
            .unwrap();
        Self {
            tasks: Some(tasks.recv().unwrap()),
            thread: Some(thread),
            log: lines,
        }
    }

    pub(crate) fn call<T: Send + 'static>(
        &self,
        task: impl FnOnce(&mut State) -> T + Send + 'static,
    ) -> T {
        let (reply, answer) = mpsc::channel();
        self.tasks
            .as_ref()
            .unwrap()
            .send(Box::new(move |state| {
                let _ = reply.send(task(state));
            }))
            .ok()
            .unwrap();
        answer.recv_timeout(PATIENCE).unwrap()
    }

    pub(crate) fn connect(&self, options: ClientOptions) -> UnixStream {
        let (ours, theirs) = UnixStream::pair().unwrap();
        self.call(move |state| {
            state.display.insert_client(theirs, options).unwrap();
        });
        ours
    }

    pub(crate) fn expect(&self, expected: &str) {
        let deadline = Instant::now() + PATIENCE;
        let mut received = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.log.recv_timeout(left) {
                Ok(line) if line == expected => return,
                Ok(line) => received.push(line),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    panic!("never saw {expected:?}; saw {received:?}")
                }
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(tasks) = self.tasks.take() {
            let _ = tasks.send(Box::new(|state: &mut State| state.stop = true));
        }
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
            && !std::thread::panicking()
        {
            panic!("the display thread panicked");
        }
    }
}
