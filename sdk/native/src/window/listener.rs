use std::sync::Arc;

use rikka_app::{SendError, Sender};

use crate::abi::{AnyThread, Closure};
use crate::ffi;
use crate::window::Model;

#[derive(Clone)]
pub(crate) struct Listener {
    sender: Sender<Model>,
    callback: Arc<Closure<ffi::ActionFn, AnyThread>>,
}

impl Listener {
    pub(crate) fn new(sender: Sender<Model>, callback: Closure<ffi::ActionFn, AnyThread>) -> Self {
        Self {
            sender,
            callback: Arc::new(callback),
        }
    }

    pub(crate) fn notify(&self) {
        let callback = Arc::clone(&self.callback);
        match self
            .sender
            .send(move |model: &mut Model| callback.call(&mut model.context()))
        {
            Ok(()) | Err(SendError) => {}
        }
    }
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").finish_non_exhaustive()
    }
}
