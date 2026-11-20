use rikka_sync::Mutex;
use std::collections::HashMap;

use futures_channel::oneshot;
use rikka_gecko_protocol::{CallbackId, Target};
use serde_json::Value;

use crate::error::Error;

pub(crate) type Outcome = Result<Value, Error>;

#[derive(Debug)]
pub(crate) struct Calls(Mutex<Table>);

#[derive(Debug)]
struct Table {
    next: CallbackId,
    pending: HashMap<CallbackId, Pending>,
    closed: bool,
}

#[derive(Debug)]
struct Pending {
    target: Target,
    sender: oneshot::Sender<Outcome>,
}

impl Pending {
    fn resolve(self, outcome: Outcome) {
        if self.sender.send(outcome).is_err() {
            tracing::trace!("engine call abandoned by its caller");
        }
    }
}

impl Default for Calls {
    fn default() -> Calls {
        Calls(Mutex::new(Table {
            next: CallbackId::FIRST,
            pending: HashMap::new(),
            closed: false,
        }))
    }
}

impl Calls {
    pub(crate) fn place(
        &self,
        target: Target,
    ) -> Result<(CallbackId, oneshot::Receiver<Outcome>), Error> {
        let mut table = self.table();
        if table.closed {
            return Err(Error::EngineExited);
        }
        let callback = table.next;
        table.next = callback.next().unwrap_or(CallbackId::FIRST);
        let (sender, receiver) = oneshot::channel();
        table.pending.insert(callback, Pending { target, sender });
        Ok((callback, receiver))
    }

    pub(crate) fn resolve(&self, callback: CallbackId, outcome: Outcome) {
        let pending = self.table().pending.remove(&callback);
        if let Some(pending) = pending {
            pending.resolve(outcome);
        }
    }

    pub(crate) fn fail_target(&self, target: &Target) {
        let failed: Vec<Pending> = {
            let mut table = self.table();
            table
                .pending
                .extract_if(|_, pending| pending.target == *target)
                .map(|(_, pending)| pending)
                .collect()
        };
        for pending in failed {
            pending.resolve(Err(Error::Closed));
        }
    }

    pub(crate) fn fail_all(&self) {
        let failed: Vec<Pending> = {
            let mut table = self.table();
            table.closed = true;
            table.pending.drain().map(|(_, pending)| pending).collect()
        };
        for pending in failed {
            pending.resolve(Err(Error::EngineExited));
        }
    }

    fn table(&self) -> rikka_sync::MutexGuard<'_, Table> {
        self.0.lock()
    }
}
