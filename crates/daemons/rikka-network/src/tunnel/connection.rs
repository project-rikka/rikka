use crate::error::{Error, Result};
use crate::profile::dbus;
use crate::tunnel::addressing;
use crate::values::{self, Draft};

pub(crate) struct Keyed<'a> {
    pub(crate) subject: &'static str,
    pub(crate) private_key: &'a str,
    pub(crate) interface: &'a str,
    pub(crate) has_peers: bool,
}

impl Keyed<'_> {
    pub(crate) fn check(&self) -> Result<()> {
        if self.private_key.trim().is_empty() {
            return Err(Error::invalid(self.subject, "it has no private key"));
        }
        if !addressing::is_interface(self.interface) {
            return Err(Error::invalid(
                self.subject,
                format!("\"{}\" is not an interface name", self.interface),
            ));
        }
        if !self.has_peers {
            return Err(Error::invalid(self.subject, "it has no peer to connect to"));
        }
        Ok(())
    }
}

pub(crate) fn draft(name: &str, kind: &str, interface: Option<&str>, user: &str) -> Draft {
    let mut draft = Draft::new();
    values::put(&mut draft, dbus::CONNECTION, "id", name.to_owned());
    values::put(&mut draft, dbus::CONNECTION, "type", kind.to_owned());
    if let Some(interface) = interface {
        values::put(
            &mut draft,
            dbus::CONNECTION,
            "interface-name",
            interface.to_owned(),
        );
    }
    values::put(&mut draft, dbus::CONNECTION, "autoconnect", false);
    values::put(
        &mut draft,
        dbus::CONNECTION,
        "permissions",
        dbus::permissions(false, user),
    );
    draft
}
