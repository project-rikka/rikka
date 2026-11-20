use std::path::Path;

use rikka_keyring::{Binding, Daemon, Served};
use rikka_secrets::{KeyringState, Secrets};
use rikka_test_bus::PrivateBus;
use tempfile::TempDir;

const PASSWORD: &[u8] = b"machine login password";

struct Bound {
    secrets: Secrets,
    _served: Served,
    _bus: PrivateBus,
}

fn bound(folder: &Path) -> Bound {
    let bus = PrivateBus::start().expect("machine tests need dbus-daemon");
    let served = zbus::block_on(
        Daemon::new(folder)
            .binding(Binding::SystemdCredentials)
            .serve(),
    )
    .expect("keyring should serve");
    let secrets = zbus::block_on(Secrets::connect()).expect("client should connect");
    Bound {
        secrets,
        _served: served,
        _bus: bus,
    }
}

fn state(bound: &Bound) -> KeyringState {
    zbus::block_on(bound.secrets.keyring().status())
        .expect("status should be readable")
        .state
}

#[test]
fn systemd_credentials_unlock_without_password() {
    let folder = TempDir::new().expect("temp folder");
    let first = bound(folder.path());
    let keyring = first.secrets.keyring();
    zbus::block_on(keyring.login(PASSWORD)).expect("login runs");
    zbus::block_on(keyring.set_auto_unlock(true)).expect("auto unlock is set");
    assert!(
        zbus::block_on(keyring.status())
            .expect("status")
            .auto_unlock
    );
    drop(first);

    let second = bound(folder.path());
    assert_eq!(state(&second), KeyringState::Unlocked);
    zbus::block_on(second.secrets.keyring().set_auto_unlock(false)).expect("auto unlock is unset");
    drop(second);

    let third = bound(folder.path());
    assert_eq!(state(&third), KeyringState::Locked);
}
