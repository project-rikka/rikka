use std::sync::Arc;

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;

use crate::error::Result;
use crate::inbox::Inbox;
use crate::notification::Notification;
use crate::posted::Posted;
use crate::reason::CloseReason;
use crate::server::{self, Server};

#[derive(Clone, Debug)]
pub struct Notifier {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    inbox: Arc<Inbox>,
    connection: Connection,
}

impl Notifier {
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> Result<Notifier> {
        let inbox = Arc::new(Inbox::new(Box::new(wake)));
        let connection = Builder::session()?
            .replace_existing_names(false)
            .allow_name_replacements(false)
            .name(server::NAME)?
            .serve_at(server::PATH, Server::new(Arc::clone(&inbox)))?
            .build()?;
        Ok(Notifier {
            inner: Arc::new(Inner { inbox, connection }),
        })
    }

    #[must_use]
    pub fn drain(&self) -> Vec<Posted> {
        self.inner.inbox.drain()
    }

    pub fn post(&self, notification: Notification) -> u32 {
        self.inner.inbox.post(notification.id, notification)
    }

    pub fn close(&self, id: u32, reason: CloseReason) {
        if !self.inner.inbox.close(id) {
            return;
        }
        self.emit("NotificationClosed", &(id, reason.code()));
    }

    #[must_use]
    pub fn is_open(&self, id: u32) -> bool {
        self.inner.inbox.is_open(id)
    }

    pub fn activate(&self, id: u32, action: &str, token: Option<&str>) {
        if !self.inner.inbox.is_open(id) {
            return;
        }
        if let Some(token) = token.filter(|token| !token.is_empty()) {
            self.emit("ActivationToken", &(id, token));
        }
        self.emit("ActionInvoked", &(id, action));
    }

    fn emit<B>(&self, signal: &str, body: &B)
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        let sent = self.inner.connection.emit_signal(
            None::<&str>,
            server::PATH,
            server::NAME,
            signal,
            body,
        );
        if let Err(error) = sent {
            tracing::warn!(%error, signal, "failed to emit signal");
        }
    }
}
