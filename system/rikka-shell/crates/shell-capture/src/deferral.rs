use crate::aim::Mode;
use crate::recorder::Subject;

#[derive(Debug)]
pub(crate) enum Deferred {
    Picture,
    Video,
    Pick(Mode),
    Record(Subject),
}

#[derive(Debug, Default)]
pub(crate) struct Deferrals {
    closing: bool,
    held: Vec<Deferred>,
}

impl Deferrals {
    pub(crate) fn close(&mut self) {
        self.closing = true;
    }

    #[must_use]
    pub(crate) fn is_closing(&self) -> bool {
        self.closing
    }

    pub(crate) fn hold(&mut self, deferred: Deferred) {
        match deferred {
            Deferred::Video => {
                if !self.disarm() {
                    self.held.push(deferred);
                }
            }
            Deferred::Pick(_) => {
                self.disarm();
                self.held.push(deferred);
            }
            Deferred::Picture | Deferred::Record(_) => self.held.push(deferred),
        }
    }

    pub(crate) fn release(&mut self) -> Vec<Deferred> {
        self.closing = false;
        std::mem::take(&mut self.held)
    }

    fn disarm(&mut self) -> bool {
        let held = self.held.len();
        self.held
            .retain(|deferred| !matches!(deferred, Deferred::Record(_)));
        self.held.len() != held
    }
}
