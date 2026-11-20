use std::convert::Infallible;
use std::sync::mpsc::{Receiver, Sender, channel};

use rikka_worker::{Stop, Worker};

use crate::event_sink::EventSink;
use crate::monitor::Apply;

pub type StartError = Box<dyn std::error::Error + Send + Sync>;

pub trait Handler: Sized + Send + 'static {
    type Config: Send + 'static;
    type Request: Send + 'static;
    type Notice: Send + 'static;
    type Event: Clone + Send + 'static;
    type State: Apply<Self::Event> + Clone + Default + std::fmt::Debug + Send + Sync + 'static;

    const THREAD_NAME: &'static str;
    const UNAVAILABLE: &'static str;

    fn open(
        config: Self::Config,
        notifier: Notifier<Self>,
        events: &EventSink<Self::Event>,
    ) -> Result<Self, StartError>;

    fn handle_request(&mut self, request: Self::Request, events: &EventSink<Self::Event>);

    fn handle_notice(&mut self, notice: Self::Notice, events: &EventSink<Self::Event>);

    fn replaces(_pending: &Self::Request, _next: &Self::Request) -> bool {
        false
    }
}

pub struct Notifier<H: Handler>(Sender<Job<H>>);

impl<H: Handler> Notifier<H> {
    pub fn notify(&self, notice: H::Notice) -> bool {
        self.0.send(Job::Notice(notice)).is_ok()
    }
}

impl<H: Handler> Clone for Notifier<H> {
    fn clone(&self) -> Notifier<H> {
        Notifier(self.0.clone())
    }
}

impl<H: Handler> std::fmt::Debug for Notifier<H> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Notifier").finish_non_exhaustive()
    }
}

pub struct Service<H: Handler> {
    worker: Worker<Jobs<H>>,
}

impl<H: Handler> Service<H> {
    pub fn start(
        config: H::Config,
        events: EventSink<H::Event>,
        unavailable: impl FnOnce(StartError) + Send + 'static,
    ) -> Result<Service<H>, StartError> {
        let (sender, jobs) = channel();
        let notifier = Notifier(sender.clone());
        let worker = Worker::spawn(H::THREAD_NAME, Jobs(sender), move || {
            Ok::<_, Infallible>(move || match H::open(config, notifier, &events) {
                Ok(handler) => run(handler, &jobs, &events),
                Err(error) => unavailable(error),
            })
        })
        .map_err(|error| -> StartError { Box::new(error) })?;
        Ok(Service { worker })
    }

    pub fn send(&self, request: H::Request) {
        if self.worker.handle().0.send(Job::Request(request)).is_err() {
            tracing::debug!(service = H::THREAD_NAME, "request dropped: worker stopped");
        }
    }
}

impl<H: Handler> std::fmt::Debug for Service<H> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Service")
            .field("name", &H::THREAD_NAME)
            .finish()
    }
}

enum Job<H: Handler> {
    Request(H::Request),
    Notice(H::Notice),
    Stop,
}

struct Jobs<H: Handler>(Sender<Job<H>>);

impl<H: Handler> Stop for Jobs<H> {
    fn stop(&self) {
        if self.0.send(Job::Stop).is_err() {
            tracing::trace!(service = H::THREAD_NAME, "worker already stopped");
        }
    }
}

fn run<H: Handler>(mut handler: H, jobs: &Receiver<Job<H>>, events: &EventSink<H::Event>) {
    while let Ok(job) = jobs.recv() {
        for job in drain_coalesced(job, jobs) {
            match job {
                Job::Stop => return,
                Job::Request(request) => handler.handle_request(request, events),
                Job::Notice(notice) => handler.handle_notice(notice, events),
            }
        }
    }
}

fn drain_coalesced<H: Handler>(first: Job<H>, jobs: &Receiver<Job<H>>) -> Vec<Job<H>> {
    let mut batch = vec![first];
    for next in jobs.try_iter() {
        let replaces = matches!(
            (&next, batch.last()),
            (Job::Request(next), Some(Job::Request(pending))) if H::replaces(pending, next)
        );
        if replaces {
            batch.pop();
        }
        batch.push(next);
    }
    batch
}
