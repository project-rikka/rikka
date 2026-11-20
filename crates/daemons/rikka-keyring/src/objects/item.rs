use std::collections::HashMap;
use std::sync::Arc;

use rikka_secrets_protocol::WireSecret;
use zbus::message::Header;
use zbus::zvariant::OwnedObjectPath;

use crate::fault::{Answer, Fault};
use crate::objects::caller;
use crate::paths;
use crate::shared::Shared;
use crate::state::State;
use rikka_keyring_vault::Item;

pub(crate) struct ItemObject {
    shared: Arc<Shared>,
    collection: String,
    id: u64,
}

impl ItemObject {
    pub(crate) fn new(shared: Arc<Shared>, collection: String, id: u64) -> ItemObject {
        ItemObject {
            shared,
            collection,
            id,
        }
    }

    async fn look<T: Default>(&self, look: impl FnOnce(&Item) -> T) -> T {
        self.shared
            .read(|state| {
                state
                    .item(&self.collection, self.id)
                    .map(look)
                    .unwrap_or_default()
            })
            .await
    }

    async fn edit(
        &self,
        edit: impl FnOnce(&mut State, &str, u64) -> Answer<()> + Send + 'static,
    ) -> zbus::fdo::Result<()> {
        let (collection, id) = (self.collection.clone(), self.id);
        self.shared
            .run(move |state| edit(state, &collection, id))
            .await
            .map_err(Fault::into_fdo)
    }
}

#[zbus::interface(name = "org.freedesktop.Secret.Item")]
impl ItemObject {
    #[zbus(out_args("Prompt"))]
    async fn delete(&self) -> Result<OwnedObjectPath, Fault> {
        let (collection, id) = (self.collection.clone(), self.id);
        self.shared
            .run(move |state| state.delete_item(&collection, id))
            .await?;
        Ok(paths::none())
    }

    #[zbus(out_args("secret"))]
    async fn get_secret(
        &self,
        session: OwnedObjectPath,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(WireSecret,), Fault> {
        let caller = caller(&header)?;
        let (collection, id) = (self.collection.clone(), self.id);
        self.shared
            .run(move |state| state.secret(&collection, id, &session, &caller))
            .await
            .map(|secret| (secret,))
    }

    async fn set_secret(
        &self,
        secret: WireSecret,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Fault> {
        let caller = caller(&header)?;
        let (collection, id) = (self.collection.clone(), self.id);
        self.shared
            .run(move |state| state.set_secret(&collection, id, &secret, &caller))
            .await
    }

    #[zbus(property)]
    async fn locked(&self) -> bool {
        self.shared
            .read(|state| state.item(&self.collection, self.id).is_err())
            .await
    }

    #[zbus(property)]
    async fn attributes(&self) -> HashMap<String, String> {
        self.look(|item| {
            item.attributes()
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()
        })
        .await
    }

    #[zbus(property)]
    async fn set_attributes(
        &mut self,
        attributes: HashMap<String, String>,
    ) -> zbus::fdo::Result<()> {
        self.edit(move |state, collection, id| {
            state.set_item_attributes(collection, id, attributes.into_iter().collect())
        })
        .await
    }

    #[zbus(property)]
    async fn label(&self) -> String {
        self.look(|item| item.label().to_owned()).await
    }

    #[zbus(property)]
    async fn set_label(&mut self, label: String) -> zbus::fdo::Result<()> {
        self.edit(move |state, collection, id| state.set_item_label(collection, id, &label))
            .await
    }

    #[zbus(property)]
    async fn created(&self) -> u64 {
        self.look(Item::created).await
    }

    #[zbus(property)]
    async fn modified(&self) -> u64 {
        self.look(Item::modified).await
    }
}
