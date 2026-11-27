use std::sync::Arc;
use std::thread::JoinHandle;

use rikka_sync::{Condvar, Mutex};

#[derive(Debug)]
struct State<T> {
    tasks: Vec<T>,
    closed: bool,
}

#[derive(Debug)]
struct Shared<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
}

#[derive(Debug)]
pub(crate) struct Queue<T>(Arc<Shared<T>>);

impl<T> Clone for Queue<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> Queue<T> {
    pub(crate) fn push(&self, task: T) {
        let mut state = self.0.state.lock();
        if state.closed {
            return;
        }
        state.tasks.push(task);
        self.0.ready.notify_one();
    }

    fn next(&self) -> Option<T> {
        let mut state = self.0.state.lock();
        loop {
            if state.closed {
                return None;
            }
            if let Some(task) = state.tasks.pop() {
                return Some(task);
            }
            state = self.0.ready.wait(state);
        }
    }

    fn close(&self) {
        let mut state = self.0.state.lock();
        state.closed = true;
        state.tasks.clear();
        self.0.ready.notify_all();
    }
}

#[derive(Debug)]
pub(crate) struct Pool<T> {
    queue: Queue<T>,
    workers: Vec<JoinHandle<()>>,
}

impl<T: Send + 'static> Pool<T> {
    pub(crate) fn new(
        name: &str,
        threads: usize,
        run: impl Fn(T) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let queue = Queue(Arc::new(Shared {
            state: Mutex::new(State {
                tasks: Vec::new(),
                closed: false,
            }),
            ready: Condvar::new(),
        }));
        let run = Arc::new(run);
        let mut pool = Self {
            queue,
            workers: Vec::with_capacity(threads),
        };
        for _ in 0..threads {
            let queue = pool.queue.clone();
            let run = Arc::clone(&run);
            let worker = std::thread::Builder::new()
                .name(name.to_owned())
                .spawn(move || {
                    while let Some(task) = queue.next() {
                        run(task);
                    }
                })?;
            pool.workers.push(worker);
        }
        Ok(pool)
    }

    pub(crate) fn queue(&self) -> Queue<T> {
        self.queue.clone()
    }
}

impl<T> Drop for Pool<T> {
    fn drop(&mut self) {
        self.queue.close();
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                tracing::error!("booth loader panicked");
            }
        }
    }
}
