use std::sync::Arc;

use rikka_secrets_protocol::WireSecret;
use zbus::message::Header;
use zbus::zvariant::OwnedObjectPath;

use crate::fault::Fault;
use crate::objects::caller;
use crate::shared::Shared;

pub(crate) struct KeyringObject {
    shared: Arc<Shared>,
}

impl KeyringObject {
    pub(crate) fn new(shared: Arc<Shared>) -> KeyringObject {
        KeyringObject { shared }
    }
}

#[zbus::interface(name = "os.rikka.Keyring1")]
impl KeyringObject {
    #[zbus(out_args("outcome"))]
    async fn login(
        &self,
        password: WireSecret,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<String, Fault> {
        let caller = caller(&header)?;
        self.shared
            .run(move |state| {
                let password = state.reveal(&password, &caller)?;
                state.login(&password)
            })
            .await
            .map(|outcome| outcome.name().to_owned())
    }

    #[zbus(out_args("opened"))]
    async fn answer(
        &self,
        prompt: OwnedObjectPath,
        password: WireSecret,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<bool, Fault> {
        let caller = caller(&header)?;
        self.shared
            .run(move |state| {
                let password = state.reveal(&password, &caller)?;
                state.answer(&prompt, &password)
            })
            .await
    }

    async fn reset(&self) -> Result<(), Fault> {
        self.shared.run(|state| state.reset()).await
    }

    async fn set_auto_unlock(&self, enabled: bool) {
        self.shared
            .run(move |state| state.set_auto_unlock(enabled))
            .await;
    }

    #[zbus(property)]
    async fn state(&self) -> String {
        self.shared
            .read(|state| state.keyring_state().name().to_owned())
            .await
    }

    #[zbus(property)]
    async fn auto_unlock(&self) -> bool {
        self.shared.read(|state| state.auto_unlock()).await
    }

    #[zbus(property)]
    async fn asking(&self) -> Vec<OwnedObjectPath> {
        self.shared.read(|state| state.asking()).await
    }
}
