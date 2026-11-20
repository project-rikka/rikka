use crate::error::{Error, Result};
use crate::profile::dbus;
use crate::request::{Join, JoinTarget};
use crate::ssid::Ssid;
use crate::values::{self, Draft};

pub(crate) fn draft(join: &Join, name: &Ssid, user: &str) -> Result<Draft> {
    let hidden = matches!(join.network, JoinTarget::Hidden(_));
    if hidden && name.is_empty() {
        return Err(Error::invalid("the name of a network", "it is empty"));
    }

    let mut draft = Draft::new();
    values::put(&mut draft, dbus::CONNECTION, "id", name.name().into_owned());
    values::put(
        &mut draft,
        dbus::CONNECTION,
        "type",
        dbus::WIRELESS.to_owned(),
    );
    values::put(
        &mut draft,
        dbus::CONNECTION,
        "permissions",
        dbus::permissions(join.shared, user),
    );
    values::put(&mut draft, dbus::WIRELESS, "ssid", name.as_bytes().to_vec());
    if hidden {
        values::put(&mut draft, dbus::WIRELESS, "hidden", true);
        values::put(
            &mut draft,
            dbus::WIRELESS,
            "mode",
            "infrastructure".to_owned(),
        );
    }

    if let Some(account) = &join.account {
        if !account.is_complete() {
            return Err(Error::invalid(
                "the account of a network",
                "it is not filled in",
            ));
        }
        let key_management = join
            .security
            .key_management()
            .unwrap_or("wpa-eap")
            .to_owned();
        values::put(&mut draft, dbus::SECURITY, "key-mgmt", key_management);
        dbus::write_eap(&mut draft, account);
        return Ok(draft);
    }

    if join.security.needs_passphrase() {
        if join.passphrase.is_empty() {
            return Err(Error::invalid("the passphrase of a network", "it is empty"));
        }
        dbus::write_passphrase(&mut draft, join.security, &join.passphrase);
    }
    if hidden && let Some(key_management) = join.security.key_management() {
        values::put(
            &mut draft,
            dbus::SECURITY,
            "key-mgmt",
            key_management.to_owned(),
        );
    }
    Ok(draft)
}
