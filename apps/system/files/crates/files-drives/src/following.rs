use std::convert::Infallible;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use rikka_bus::{ManagedObjects, ObjectChanges, Watch};
use rikka_sync::Mutex;
use rikka_worker::{Stop, Worker};
use zbus::blocking::Connection;

use crate::udisks::{ROOT, SERVICE};

const THREAD_NAME: &str = "rikka-files-drives";

pub(crate) type Cache = Arc<Mutex<Option<ManagedObjects>>>;

#[derive(Debug)]
pub(crate) struct Following {
    _watch: Watch,
    _worker: Worker<Jobs>,
}

enum Job {
    Changes(ObjectChanges),
    Stop,
}

struct Jobs(Sender<Job>);

impl Stop for Jobs {
    fn stop(&self) {
        if self.0.send(Job::Stop).is_err() {
            tracing::trace!("udisks2 follower already stopped");
        }
    }
}

impl Following {
    pub(crate) fn start(
        connection: &Connection,
        cache: Cache,
        changed: impl Fn() + Send + 'static,
    ) -> Option<Following> {
        let (sender, jobs) = channel();
        let heard = sender.clone();
        let watch = ManagedObjects::watch(connection, SERVICE, ROOT, move |changes| {
            heard.send(Job::Changes(changes)).is_ok()
        })
        .inspect_err(|error| tracing::warn!(%error, "cannot watch udisks2"))
        .ok()?;
        let connection = connection.clone();
        let worker = Worker::spawn(THREAD_NAME, Jobs(sender), move || {
            Ok::<_, Infallible>(move || follow(&connection, &cache, &jobs, &changed))
        })
        .inspect_err(|error| tracing::warn!(%error, "cannot follow udisks2"))
        .ok()?;
        Some(Following {
            _watch: watch,
            _worker: worker,
        })
    }
}

fn follow(connection: &Connection, cache: &Cache, jobs: &Receiver<Job>, changed: &impl Fn()) {
    while let Ok(Job::Changes(changes)) = jobs.recv() {
        let moved = {
            let mut objects = cache.lock();
            match objects
                .as_mut()
                .map(|objects| objects.follow(connection, changes))
            {
                Some(Ok(moved)) => moved,
                Some(Err(error)) => {
                    tracing::debug!(%error, "failed to follow udisks2");
                    *objects = None;
                    true
                }
                None => true,
            }
        };
        if moved {
            changed();
        }
    }
}
