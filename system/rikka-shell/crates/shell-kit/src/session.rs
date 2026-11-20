use std::thread;

use rikka_seam::net::SocketClient;
use rikka_seam::{Delivery, Seam, SeamClient};

use crate::host::Host;
use crate::waker::WakingSender;

#[derive(Debug)]
pub struct Session {
    connection: Option<SocketClient>,
    relay: Option<thread::JoinHandle<()>>,
}

impl Session {
    pub(crate) fn connect(seam: Seam<Host>) -> (Seam<Host>, Option<Self>) {
        let Ok(connection) = SocketClient::from_env().inspect_err(|reason| {
            tracing::debug!("no session socket: {reason}");
        }) else {
            return (seam, None);
        };
        tracing::info!("connected to session socket");
        let seam = seam.forwarding_to(connection.dispatcher());
        (
            seam,
            Some(Self {
                connection: Some(connection),
                relay: None,
            }),
        )
    }

    pub(crate) fn client(&self) -> Option<SeamClient> {
        self.connection.as_ref().map(SocketClient::client)
    }

    pub fn relay_into(&mut self, channel: WakingSender<Delivery>) {
        let Some(connection) = &self.connection else {
            return;
        };
        let mut events = connection.events();
        self.relay = thread::Builder::new()
            .name("rikka-shell-seam".to_owned())
            .spawn(move || {
                while let Some(event) = events.blocking_next() {
                    if channel.post(event).is_err() {
                        break;
                    }
                }
            })
            .inspect_err(|error| {
                tracing::warn!("failed to subscribe to session announcements: {error}");
            })
            .ok();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        drop(self.connection.take());
        if let Some(relay) = self.relay.take()
            && relay.join().is_err()
        {
            tracing::warn!("session event relay thread panicked");
        }
    }
}
