use std::sync::mpsc::{self, Receiver, Sender};

use crate::error::{Error, Result};

pub(crate) struct StartupSender<E>(Sender<std::result::Result<(), E>>);

pub(crate) struct StartupReceiver<E>(Receiver<std::result::Result<(), E>>);

pub(crate) fn open<E>() -> (StartupSender<E>, StartupReceiver<E>) {
    let (sender, receiver) = mpsc::channel();
    (StartupSender(sender), StartupReceiver(receiver))
}

impl<E> StartupSender<E> {
    pub(crate) fn report_running(self) {
        let _ = self.0.send(Ok(()));
    }

    pub(crate) fn report_failure(self, error: E) {
        let _ = self.0.send(Err(error));
    }
}

impl<E> StartupReceiver<E> {
    pub(crate) fn wait(self) -> Result<(), E> {
        match self.0.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(Error::Startup(error)),
            Err(_) => Err(Error::Exited),
        }
    }
}
