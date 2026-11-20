use futures_channel::mpsc;

use crate::voice::update::{VoiceUpdate, VoiceUpdates};

#[derive(Clone, Debug)]
pub(crate) struct Outbox(mpsc::UnboundedSender<VoiceUpdate>);

impl Outbox {
    pub(crate) fn new() -> (Outbox, VoiceUpdates) {
        let (sender, receiver) = mpsc::unbounded();
        (Outbox(sender), receiver)
    }

    pub(crate) fn send(&self, update: VoiceUpdate) {
        let _ = self.0.unbounded_send(update);
    }
}
