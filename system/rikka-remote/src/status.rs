use std::sync::mpsc::Sender;

use rikka_session::remote::Viewer;

use crate::controller::Message;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StatusChange {
    Viewer(Option<Viewer>),
    Problem(Option<String>),
}

#[derive(Clone, Debug)]
pub(crate) struct StatusSink(Sender<Message>);

impl StatusSink {
    pub(crate) fn new(messages: Sender<Message>) -> StatusSink {
        StatusSink(messages)
    }

    pub(crate) fn report(&self, change: StatusChange) {
        if self.0.send(Message::Status(change)).is_err() {
            tracing::debug!("status after the main loop stopped");
        }
    }
}
