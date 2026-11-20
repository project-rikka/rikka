use rikka_secrets::{Error, KeyringState, LoginOutcome};
use tempfile::TempDir;

use crate::support::{FIRST, Running, SECOND, TOKEN, finished, token};

#[test]
fn first_login_creates_keyring_that_survives_restart() {
    let folder = TempDir::new().expect("temp folder");
    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    assert_eq!(running.state(), KeyringState::Absent);
    let keyring = running.secrets.keyring();
    assert_eq!(
        zbus::block_on(keyring.login(FIRST)).expect("login runs"),
        LoginOutcome::Created
    );
    zbus::block_on(running.secrets.store(&token(), "Test sign-in", TOKEN)).expect("secret stores");
    drop(running);

    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    assert_eq!(running.state(), KeyringState::Locked);
    assert!(matches!(
        zbus::block_on(running.secrets.lookup(&token())),
        Err(Error::Locked)
    ));
    assert_eq!(
        zbus::block_on(running.secrets.keyring().login(FIRST)).expect("login runs"),
        LoginOutcome::Unlocked
    );
    let secret = zbus::block_on(running.secrets.lookup(&token()))
        .expect("lookup runs")
        .expect("secret survives");
    assert_eq!(secret.expose(), TOKEN);
}

#[test]
fn open_keyring_follows_changed_login_password() {
    let folder = TempDir::new().expect("temp folder");
    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    let keyring = running.secrets.keyring();
    zbus::block_on(keyring.login(FIRST)).expect("login runs");
    assert_eq!(
        zbus::block_on(keyring.login(SECOND)).expect("login runs"),
        LoginOutcome::Changed
    );
    drop(running);

    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    let keyring = running.secrets.keyring();
    assert_eq!(
        zbus::block_on(keyring.login(FIRST)).expect("login runs"),
        LoginOutcome::Refused
    );
    assert_eq!(
        zbus::block_on(keyring.login(SECOND)).expect("login runs"),
        LoginOutcome::Unlocked
    );
}

#[test]
fn previous_password_opens_and_rewraps_refused_keyring() {
    let folder = TempDir::new().expect("temp folder");
    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    zbus::block_on(running.secrets.keyring().login(FIRST)).expect("login runs");
    zbus::block_on(running.secrets.store(&token(), "Test sign-in", TOKEN)).expect("secret stores");
    drop(running);

    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    let keyring = running.secrets.keyring();
    assert_eq!(
        zbus::block_on(keyring.login(SECOND)).expect("login runs"),
        LoginOutcome::Refused
    );
    assert_eq!(running.state(), KeyringState::Refused);
    let (prompt, unlocked) = running.unlock_in_background();
    assert!(!zbus::block_on(keyring.answer(&prompt, b"a guess")).expect("answer runs"));
    assert!(zbus::block_on(keyring.answer(&prompt, FIRST)).expect("answer runs"));
    finished(&unlocked).expect("unlock request should succeed");
    assert_eq!(running.state(), KeyringState::Unlocked);
    drop(running);

    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    assert_eq!(
        zbus::block_on(running.secrets.keyring().login(SECOND)).expect("login runs"),
        LoginOutcome::Unlocked
    );
    let secret = zbus::block_on(running.secrets.lookup(&token()))
        .expect("lookup runs")
        .expect("secret survives");
    assert_eq!(secret.expose(), TOKEN);
}

#[test]
fn reset_starts_over_with_login_password() {
    let folder = TempDir::new().expect("temp folder");
    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    zbus::block_on(running.secrets.keyring().login(FIRST)).expect("login runs");
    zbus::block_on(running.secrets.store(&token(), "Test sign-in", TOKEN)).expect("secret stores");
    drop(running);

    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    let keyring = running.secrets.keyring();
    assert!(zbus::block_on(keyring.reset()).is_err());
    zbus::block_on(keyring.login(SECOND)).expect("login runs");
    zbus::block_on(keyring.reset()).expect("reset runs");

    assert_eq!(running.state(), KeyringState::Unlocked);
    assert!(
        zbus::block_on(running.secrets.lookup(&token()))
            .expect("lookup runs")
            .is_none()
    );
}

#[test]
fn dismissed_prompt_leaves_keyring_locked() {
    let folder = TempDir::new().expect("temp folder");
    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    zbus::block_on(running.secrets.keyring().login(FIRST)).expect("login runs");
    drop(running);

    let Some(running) = Running::start(folder.path()) else {
        return;
    };
    let (prompt, unlocked) = running.unlock_in_background();
    zbus::block_on(running.secrets.keyring().dismiss(&prompt)).expect("dismiss runs");

    assert!(matches!(finished(&unlocked), Err(Error::Dismissed)));
    assert_eq!(running.state(), KeyringState::Locked);
}
