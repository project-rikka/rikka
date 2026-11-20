use rikka_bus::login::{self, ManagerProxy};
use zbus::blocking::Connection;

use crate::error::{Error, Result};

pub(crate) fn id(connection: &Connection) -> Result<String> {
    match ask(connection) {
        Ok(id) => Ok(id),
        Err(error) => {
            tracing::debug!("logind session lookup failed ({error}), using XDG_SESSION_ID");
            std::env::var("XDG_SESSION_ID").map_err(|_| Error::Sessionless(error.to_string()))
        }
    }
}

fn ask(connection: &Connection) -> zbus::Result<String> {
    let manager = ManagerProxy::new(connection)?;
    let path = login::current_session(&manager)?;
    login::session_proxy(connection, path)?.id()
}
