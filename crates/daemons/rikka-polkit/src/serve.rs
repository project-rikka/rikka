use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use zbus::Message;
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;

use rikka_bus::polkit::{AUTHORITY, AuthorityProxyBlocking, Subject};
use rikka_bus::{DaemonError, Rule, Signals, Watch};

use crate::error::{Error, Result};
use crate::listener::{self, Listener};
use crate::pending::Pending;
use crate::session;

const BUS: &str = "org.freedesktop.DBus";
const NAME_OWNER_CHANGED: &str = "NameOwnerChanged";

pub(crate) enum Wake {
    Stop,
    Owner(String),
}

pub(crate) fn run(
    pending: Arc<Pending>,
    locale: String,
    wakes: Sender<Wake>,
    woken: Receiver<Wake>,
) {
    let (connection, session) = match serve(pending) {
        Ok(served) => served,
        Err(error) => {
            tracing::warn!("failed to register polkit agent: {error}");
            return;
        }
    };

    let _watch = watch(&connection, wakes);
    let mut known = registering(&connection, &session, &locale);

    while let Ok(Wake::Owner(owner)) = woken.recv() {
        if known.as_deref() == Some(owner.as_str()) {
            continue;
        }
        tracing::info!("polkit restarted, re-registering agent");
        known = registering(&connection, &session, &locale);
    }

    unregistering(&connection, &session);
    connection.close().ok();
}

fn watch(connection: &Connection, wakes: Sender<Wake>) -> Option<Watch> {
    Signals::new()
        .rule(
            Rule::new()
                .sender(BUS)
                .interface(BUS)
                .member(NAME_OWNER_CHANGED)
                .arg(0, AUTHORITY),
        )
        .each(connection, owner, move |owner| {
            wakes.send(Wake::Owner(owner)).is_ok()
        })
        .inspect_err(|error| tracing::warn!("failed to watch polkit owner changes: {error}"))
        .ok()
}

fn owner(message: &Message) -> Option<String> {
    let body = message.body();
    let (_, _, owner) = body.deserialize::<(&str, &str, &str)>().ok()?;
    (!owner.is_empty()).then(|| owner.to_owned())
}

fn registering(connection: &Connection, session: &str, locale: &str) -> Option<String> {
    match register(connection, session, locale) {
        Ok(registered) => {
            tracing::info!("registered polkit authentication agent");
            registered
        }
        Err(error) => {
            tracing::warn!("failed to register polkit agent: {error}");
            None
        }
    }
}

fn register(connection: &Connection, session: &str, locale: &str) -> Result<Option<String>> {
    let subject = Subject::session(session)?;
    let result = AuthorityProxyBlocking::new(connection)?
        .inner()
        .call_method(
            "RegisterAuthenticationAgent",
            &(&subject, locale, listener::PATH),
        )?;
    Ok(result.header().sender().map(ToString::to_string))
}

fn unregistering(connection: &Connection, session: &str) {
    let unregister = || -> Result<()> {
        let subject = Subject::session(session)?;
        AuthorityProxyBlocking::new(connection)?
            .unregister_authentication_agent(&subject, listener::PATH)?;
        Ok(())
    };
    if let Err(error) = unregister() {
        tracing::debug!("failed to unregister polkit agent: {error}");
    }
}

fn serve(pending: Arc<Pending>) -> Result<(Connection, String)> {
    let connection = Builder::system()
        .map_err(Error::unreachable)?
        .serve_at(listener::PATH, Listener::new(pending))
        .map_err(Error::unreachable)?
        .build()
        .map_err(Error::unreachable)?;
    let session = session::id(&connection)?;
    Ok((connection, session))
}
