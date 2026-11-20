use std::convert::Infallible;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use rikka_bus::DaemonError;
use rikka_worker::{Stop, Worker};

use crate::ask::Ask;
use crate::error::{Error, Result};
use crate::pending::Pending;
use crate::reply::Reply;
use crate::serve::{self, Wake};

#[derive(Clone, Debug)]
pub struct Agent {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    pending: Arc<Pending>,
    worker: Worker<Halt>,
}

#[derive(Debug)]
struct Halt(Sender<Wake>);

impl Stop for Halt {
    fn stop(&self) {
        let _ = self.0.send(Wake::Stop);
    }
}

impl Agent {
    pub fn start(report: impl Fn(Option<Box<Ask>>) + Send + Sync + 'static) -> Result<Agent> {
        let pending = Arc::new(Pending::new(Arc::new(report)));
        let (wakes, woken) = std::sync::mpsc::channel();

        let serving = Arc::clone(&pending);
        let locale = locale();
        let owners = wakes.clone();
        let worker = Worker::spawn("rikka-polkit", Halt(wakes), move || {
            Ok::<_, Infallible>(move || serve::run(serving, locale, owners, woken))
        })
        .map_err(Error::unreachable)?;

        Ok(Agent {
            inner: Arc::new(Inner { pending, worker }),
        })
    }

    pub fn answer(&self, ask: u64, reply: Reply) {
        self.inner.pending.answer(ask, reply);
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.pending.close();
        self.worker.stop();
    }
}

fn locale() -> String {
    rikka_types::LocaleCategory::Messages
        .posix_name()
        .unwrap_or_else(|| "C".to_owned())
}
