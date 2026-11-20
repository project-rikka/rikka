mod commands;
mod server;

use std::fmt;

use rikka_compositor_xwm::{Result as XResult, Xwm};

use server::XServer;

#[derive(Default)]
pub(crate) struct X11 {
    server: Option<XServer>,
}

impl X11 {
    pub(crate) fn get(&self) -> Option<&Xwm> {
        self.server.as_ref().and_then(XServer::manager)
    }

    pub(crate) fn display_number(&self) -> Option<u32> {
        self.server.as_ref().map(XServer::display_number)
    }

    fn command<T>(&mut self, run: impl FnOnce(&mut Xwm) -> XResult<T>) -> Option<XResult<T>> {
        self.server.as_mut().and_then(XServer::manager_mut).map(run)
    }
}

impl fmt::Debug for X11 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.server {
            Some(server) => server.fmt(formatter),
            None => formatter.write_str("X11(absent)"),
        }
    }
}
