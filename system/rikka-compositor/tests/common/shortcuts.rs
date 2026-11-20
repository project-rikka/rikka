use std::time::Instant;

use rikka_seam::net::{SocketClient, socket_path_for};
use rikka_seam::{EventQueue, Request, SeamClient};
use rikka_session::shortcuts::{ACTIVATED, CAPABILITY, Shortcut};
use rikka_session::workspaces;
use serde::de::DeserializeOwned;

use super::{PATIENCE, SOCKET};

pub(crate) struct Announced {
    seam: SocketClient,
    events: EventQueue,
}

impl Announced {
    pub(crate) fn listen() -> Self {
        let (announced, _): (Self, serde_json::Value) =
            Self::subscribe(Request::new(workspaces::CAPABILITY, workspaces::STATE));
        announced
    }

    pub(crate) fn subscribe<T: DeserializeOwned>(barrier: Request) -> (Self, T) {
        let path = socket_path_for(SOCKET).expect("runtime directory set");
        let seam = SocketClient::connect(path).expect("session seam socket connects");
        let events = seam.events();
        let reply = seam
            .client()
            .call(barrier)
            .blocking_wait()
            .and_then(|payload| payload.decode())
            .expect("session seam reply after subscribing");
        (Self { seam, events }, reply)
    }

    pub(crate) fn client(&self) -> SeamClient {
        self.seam.client()
    }

    pub(crate) fn next(&mut self, what: &str) -> Shortcut {
        self.heard(CAPABILITY, ACTIVATED, what)
    }

    pub(crate) fn heard<T: DeserializeOwned>(
        &mut self,
        capability: &str,
        name: &str,
        what: &str,
    ) -> T {
        self.until(capability, name, what, |_: &T| true)
    }

    pub(crate) fn until<T: DeserializeOwned>(
        &mut self,
        capability: &str,
        name: &str,
        what: &str,
        mut accept: impl FnMut(&T) -> bool,
    ) -> T {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let delivery = self
                .events
                .next_timeout(remaining)
                .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
            let Some(event) = delivery.event() else {
                continue;
            };
            if event.capability().as_str() != capability || event.name() != name {
                continue;
            }
            let value: T = event.decode().expect("event payload matches its type");
            if accept(&value) {
                return value;
            }
        }
    }
}
