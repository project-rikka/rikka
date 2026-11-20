use std::path::PathBuf;
use std::sync::OnceLock;

use rikka_keyring::{Daemon, Served};
use rikka_secrets_protocol::WireSecret;
use rikka_secrets_protocol::names::{
    BUS_NAME, KEYRING_INTERFACE, SERVICE_INTERFACE, SERVICE_PATH, TEXT_CONTENT,
};
use rikka_test_bus::PrivateBus;
use tempfile::TempDir;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

pub const PASSWORD: &[u8] = b"rikka test login password";

const KEYRINGS: &str = "keyrings";

static SHARED: OnceLock<Option<TestKeyring>> = OnceLock::new();

#[derive(Debug)]
pub struct TestKeyring {
    folder: TempDir,
    _served: Served,
    _bus: PrivateBus,
}

impl TestKeyring {
    #[must_use]
    pub fn unlocked() -> Option<&'static TestKeyring> {
        SHARED.get_or_init(TestKeyring::start).as_ref()
    }

    #[must_use]
    pub fn folder(&self) -> PathBuf {
        self.folder.path().join(KEYRINGS)
    }

    fn start() -> Option<TestKeyring> {
        let bus = PrivateBus::start()?;
        let folder = TempDir::new().expect("keyring folder should be created");
        let served = zbus::block_on(Daemon::new(folder.path().join(KEYRINGS)).serve())
            .expect("keyring should serve on the private bus");
        zbus::block_on(log_in(PASSWORD)).expect("login keyring should be created");
        Some(TestKeyring {
            folder,
            _served: served,
            _bus: bus,
        })
    }
}

async fn log_in(password: &[u8]) -> zbus::Result<String> {
    let client = zbus::connection::Builder::session()?.build().await?;
    let service = zbus::Proxy::new(&client, BUS_NAME, SERVICE_PATH, SERVICE_INTERFACE).await?;
    let (_, session): (OwnedValue, OwnedObjectPath) = service
        .call("OpenSession", &("plain", Value::from("")))
        .await?;
    let keyring = zbus::Proxy::new(&client, BUS_NAME, SERVICE_PATH, KEYRING_INTERFACE).await?;
    keyring
        .call(
            "Login",
            &(WireSecret {
                session,
                parameters: Vec::new(),
                value: password.to_vec(),
                content_type: TEXT_CONTENT.to_owned(),
            },),
        )
        .await
}
