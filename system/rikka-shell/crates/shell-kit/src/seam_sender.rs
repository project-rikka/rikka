use rikka_seam::{Call, Dispatcher, SeamError};

use crate::waker::WakingSender;

#[derive(Debug)]
pub(crate) struct SeamSender(WakingSender<Call>);

impl SeamSender {
    pub(crate) fn new(channel: WakingSender<Call>) -> Self {
        Self(channel)
    }
}

impl Dispatcher for SeamSender {
    fn enqueue(&self, call: Call) {
        if let Err(returned) = self.0.post(call) {
            returned.fail(SeamError::Disconnected);
        }
    }
}
