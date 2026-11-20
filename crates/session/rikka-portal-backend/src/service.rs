use zbus::blocking::{Connection, connection};
use zbus::object_server::Interface;

const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub fn serve<I: Interface>(name: &'static str, interface: I) -> zbus::Result<Connection> {
    connection::Builder::session()?
        .name(name)?
        .serve_at(PORTAL_PATH, interface)?
        .build()
}
