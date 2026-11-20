use std::sync::Arc;

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;

use crate::composing::Composing;
use crate::error::{Error, Result};
use crate::impanel::Impanel;
use crate::inbox::Inbox;
use crate::listen;
use crate::names;
use crate::state::State;
use crate::status_area::StatusArea;
use rikka_bus::Watch;

#[derive(Clone, Debug)]
pub struct Panel {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    inbox: Arc<Inbox>,
    connection: Connection,
    _listening: Watch,
}

impl Panel {
    pub fn start(wake: impl Fn() + Send + Sync + 'static) -> Result<Panel> {
        let inbox = Arc::new(Inbox::new(Box::new(wake)));
        let connection = Builder::session()
            .map_err(Error::bus)?
            .replace_existing_names(false)
            .allow_name_replacements(false)
            .name(names::PANEL)
            .map_err(Error::bus)?
            .serve_at(names::PANEL_PATH, Impanel::new(Arc::clone(&inbox)))
            .map_err(Error::bus)?
            .build()
            .map_err(name_taken_error)?;

        let listening = listen::start(&connection, &inbox)?;
        listen::announce_panel(&connection);
        Ok(Panel {
            inner: Arc::new(Inner {
                inbox,
                connection,
                _listening: listening,
            }),
        })
    }

    #[must_use]
    pub fn composing(&self) -> Arc<Composing> {
        self.inner.inbox.composing()
    }

    #[must_use]
    pub fn is_composing(&self) -> bool {
        self.inner.inbox.is_composing()
    }

    #[must_use]
    pub fn status_area(&self) -> Arc<StatusArea> {
        self.inner.inbox.status_area()
    }

    pub fn close_menu(&self) {
        self.inner.inbox.update(State::fold);
    }

    pub fn trigger(&self, key: &str) -> Result<()> {
        self.inner.inbox.update(|state| state.ask(key));
        self.inner
            .connection
            .emit_signal(
                None::<&str>,
                names::PANEL_PATH,
                names::PANEL_V1,
                names::TRIGGER_PROPERTY,
                &key,
            )
            .map_err(|error| Error::Refused {
                call: names::TRIGGER_PROPERTY,
                reason: error.to_string(),
            })
    }
}

fn name_taken_error(error: zbus::Error) -> Error {
    match error {
        zbus::Error::NameTaken => Error::Taken(names::PANEL),
        error => Error::bus(error),
    }
}
