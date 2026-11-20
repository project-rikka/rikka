use std::sync::mpsc::Receiver;

use rikka_seam::net::SocketListener;
use rikka_seam::{Call, Delivery, EventQueue, Seam};

use shell_kit::Host;
use shell_kit::hosting::Domains;
use shell_kit::hosting::LoopWaker;
use shell_kit::hosting::Session;
use shell_kit::hosting::Watcher;

pub(super) struct Bridge {
    seam: Seam<Host>,
    calls: Receiver<Call>,
    incoming: Receiver<Delivery>,
    announcements: Option<EventQueue>,
    watchers: Vec<Watcher>,
    _session: Option<Session>,
    _door: Option<SocketListener>,
    domains: Domains,
}

pub(super) struct BridgeParts {
    pub(crate) seam: Seam<Host>,
    pub(crate) calls: Receiver<Call>,
    pub(crate) watchers: Vec<Watcher>,
    pub(crate) session: Option<Session>,
    pub(crate) domains: Domains,
}

impl Bridge {
    pub(crate) fn new(ends: BridgeParts, waker: &LoopWaker) -> Bridge {
        let BridgeParts {
            seam,
            calls,
            watchers,
            mut session,
            domains,
        } = ends;
        let (relayed, incoming) = waker.channel::<Delivery>();
        if let Some(session) = &mut session {
            session.relay_into(relayed);
        }
        let announcements = (!watchers.is_empty() || domains.is_followed())
            .then(|| seam.client().poll_events(None::<&str>));
        let door = crate::door::open(seam.client());
        Bridge {
            seam,
            calls,
            incoming,
            announcements,
            watchers,
            _session: session,
            _door: door,
            domains,
        }
    }

    pub(super) fn read_mail(&mut self, host: &mut Host) {
        while let Ok(call) = self.calls.try_recv() {
            self.seam.dispatch(host, call);
        }
        while let Ok(incoming) = self.incoming.try_recv() {
            self.seam.relay(incoming);
        }
    }

    pub(crate) fn announce(&mut self, host: &mut Host) {
        let Some(announcements) = &mut self.announcements else {
            return;
        };
        while let Some(incoming) = announcements.try_next() {
            self.domains.deliver(host, &incoming);
            for watcher in &self.watchers {
                let theirs = match &incoming {
                    Delivery::Event(event) => watcher.capability == *event.capability(),
                    Delivery::Lagged(lagged) => lagged.includes(&watcher.capability),
                };
                if theirs {
                    (watcher.act)(host, &incoming);
                }
            }
        }
    }
}

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge")
            .field("seam", &self.seam)
            .field("watching", &self.watchers.len())
            .finish_non_exhaustive()
    }
}
