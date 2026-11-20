use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct Notes(Sender<String>);

impl Notes {
    pub(crate) fn channel() -> (Self, Receiver<String>) {
        let (notes, lines) = mpsc::channel();
        (Self(notes), lines)
    }

    pub fn note(&self, line: impl Into<String>) {
        let _ = self.0.send(line.into());
    }
}
