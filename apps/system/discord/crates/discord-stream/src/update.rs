use discord_core::UserId;
use futures_channel::mpsc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamUpdate {
    Viewers(Vec<UserId>),
    Paused(bool),
    Watched(bool),
    Showing { width: u32, height: u32 },
    Preview(Vec<u8>),
    Ended(Option<String>),
}

pub type StreamUpdates = mpsc::UnboundedReceiver<StreamUpdate>;

#[derive(Clone, Debug)]
pub(crate) struct Outbox(mpsc::UnboundedSender<StreamUpdate>);

impl Outbox {
    pub(crate) fn new() -> (Outbox, StreamUpdates) {
        let (sender, receiver) = mpsc::unbounded();
        (Outbox(sender), receiver)
    }

    pub(crate) fn send(&self, update: StreamUpdate) {
        let _ = self.0.unbounded_send(update);
    }
}
