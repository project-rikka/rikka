use std::path::Path;
use std::sync::mpsc;

use futures_util::StreamExt;
use rikka_keyring::{Daemon, Served};
use rikka_secrets::{Entry, KeyringState, PromptId, Secrets};
use rikka_test_bus::{PrivateBus, TIMEOUT};

pub(crate) const FIRST: &[u8] = b"first login password";
pub(crate) const SECOND: &[u8] = b"second login password";
pub(crate) const TOKEN: &[u8] = b"notarealtoken";

pub(crate) struct Running {
    pub(crate) secrets: Secrets,
    _served: Served,
    _bus: PrivateBus,
}

impl Running {
    pub(crate) fn start(folder: &Path) -> Option<Running> {
        let bus = PrivateBus::start()?;
        let served = zbus::block_on(Daemon::new(folder).serve()).expect("keyring should serve");
        let secrets = zbus::block_on(Secrets::connect()).expect("client should connect");
        Some(Running {
            secrets,
            _served: served,
            _bus: bus,
        })
    }

    pub(crate) fn state(&self) -> KeyringState {
        zbus::block_on(self.secrets.keyring().status())
            .expect("status should be readable")
            .state
    }

    pub(crate) fn unlock_in_background(
        &self,
    ) -> (PromptId, mpsc::Receiver<rikka_secrets::Result<()>>) {
        let mut changes =
            zbus::block_on(self.secrets.changes()).expect("changes should be watchable");
        let (sender, unlocked) = mpsc::channel();
        let secrets = self.secrets.clone();
        std::thread::spawn(move || {
            let _ = sender.send(zbus::block_on(secrets.unlock()));
        });
        let keyring = self.secrets.keyring();
        let prompt = zbus::block_on(async {
            loop {
                let status = keyring.status().await.expect("status should be readable");
                if let Some(prompt) = status.asking.into_iter().next() {
                    return prompt;
                }
                changes
                    .next()
                    .await
                    .expect("keyring should announce the prompt");
            }
        });
        (prompt, unlocked)
    }
}

pub(crate) fn token() -> Entry {
    Entry::new("os.rikka.Test").attribute("account", "alice")
}

pub(crate) fn finished(
    unlocked: &mpsc::Receiver<rikka_secrets::Result<()>>,
) -> rikka_secrets::Result<()> {
    unlocked
        .recv_timeout(TIMEOUT)
        .expect("the unlock request should finish")
}
