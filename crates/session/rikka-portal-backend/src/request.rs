use std::sync::Arc;

use zbus::interface;

use crate::cancel::Cancel;

#[derive(Debug)]
pub(crate) struct Request {
    cancel: Arc<Cancel>,
}

impl Request {
    pub(crate) fn new(cancel: Arc<Cancel>) -> Request {
        Request { cancel }
    }
}

#[interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    fn close(&self) {
        self.cancel.stop();
    }
}
