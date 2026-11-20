use rikka_seat::Seats;
use rikka_wayland::{Display, Host};
use rikka_wayland_client::{Connection, Queue};

use crate::error::{Error, Result};
use crate::wayland::dispatch;
use crate::wayland::globals::Globals;
use crate::wayland::outputs::Outputs;
use crate::wayland::runner::Runner;

#[derive(Debug)]
pub(crate) struct Client {
    pub(crate) connection: Connection,
    pub(crate) host: Host,
    pub(crate) globals: Globals,
    pub(crate) outputs: Outputs,
    pub(crate) seats: Seats,
}

impl Client {
    pub(crate) fn connect() -> Result<(Client, Queue<Runner>)> {
        let display = Display::from_env().map_err(|error| match error {
            rikka_wayland::Error::NoSession => Error::NoDisplay,
            other => Error::Display(other),
        })?;
        let connection = display.connection().clone();
        let mut queue = connection.queue::<Runner>();
        dispatch::listen(&mut queue);
        let route = queue.route();
        let globals = Globals::bind(&display, &route)?;
        let seats = Seats::builder()
            .gestures()
            .relative_motion()
            .bind(&display, &route);
        display.announce_globals(&route);
        Ok((
            Client {
                connection,
                host: Host::read_by_owner(display),
                globals,
                outputs: Outputs::default(),
                seats,
            },
            queue,
        ))
    }
}
