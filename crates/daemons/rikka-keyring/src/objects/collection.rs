use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rikka_secrets_protocol::WireSecret;
use rikka_secrets_protocol::names::{ITEM_ATTRIBUTES, ITEM_LABEL};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};

use crate::fault::{Answer, Fault};
use crate::objects::caller;
use crate::paths::{self, Target};
use crate::shared::Shared;
use crate::state::{Creating, State};

pub(crate) struct CollectionObject {
    shared: Arc<Shared>,
    target: Target,
}

impl CollectionObject {
    pub(crate) fn new(shared: Arc<Shared>, target: Target) -> CollectionObject {
        CollectionObject { shared, target }
    }

    fn name(&self, state: &State) -> Answer<String> {
        state
            .resolve(&self.target)
            .ok_or_else(|| Fault::no_object("collection"))
    }

    async fn look<T: Default>(&self, look: impl FnOnce(&State, &str) -> T) -> T {
        self.shared
            .read(|state| match self.name(state) {
                Ok(name) => look(state, &name),
                Err(_) => T::default(),
            })
            .await
    }
}

#[zbus::interface(name = "org.freedesktop.Secret.Collection")]
impl CollectionObject {
    #[zbus(out_args("prompt"))]
    async fn delete(&self) -> Result<OwnedObjectPath, Fault> {
        let target = self.target.clone();
        self.shared
            .run(move |state| {
                let name = state
                    .resolve(&target)
                    .ok_or_else(|| Fault::no_object("collection"))?;
                state.delete_collection(&name)
            })
            .await?;
        Ok(paths::none())
    }

    #[zbus(out_args("results"))]
    async fn search_items(&self, attributes: HashMap<String, String>) -> Vec<OwnedObjectPath> {
        let query: BTreeMap<String, String> = attributes.into_iter().collect();
        self.look(|state, name| state.search_collection(name, &query))
            .await
    }

    #[zbus(out_args("item", "prompt"))]
    async fn create_item(
        &self,
        properties: HashMap<String, OwnedValue>,
        secret: WireSecret,
        replace: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(OwnedObjectPath, OwnedObjectPath), Fault> {
        let caller = caller(&header)?;
        let label = match properties.get(ITEM_LABEL) {
            Some(value) => String::try_from(value.clone())
                .map_err(|_| Fault::invalid("the item label is not a string"))?,
            None => String::new(),
        };
        let attributes: BTreeMap<String, String> = match properties.get(ITEM_ATTRIBUTES) {
            Some(value) => HashMap::<String, String>::try_from(value.clone())
                .map_err(|_| Fault::invalid("the item attributes are not a string map"))?
                .into_iter()
                .collect(),
            None => BTreeMap::new(),
        };
        let target = self.target.clone();
        let item = self
            .shared
            .run(move |state| {
                let name = state
                    .resolve(&target)
                    .ok_or_else(|| Fault::no_object("collection"))?;
                state.create_item(
                    &name,
                    &caller,
                    Creating {
                        label,
                        attributes,
                        secret,
                        replace,
                    },
                )
            })
            .await?;
        Ok((item, paths::none()))
    }

    #[zbus(property)]
    async fn items(&self) -> Vec<OwnedObjectPath> {
        self.look(State::item_paths).await
    }

    #[zbus(property)]
    async fn label(&self) -> String {
        self.look(State::label).await
    }

    #[zbus(property)]
    async fn set_label(&mut self, label: String) -> zbus::fdo::Result<()> {
        let target = self.target.clone();
        self.shared
            .run(move |state| {
                let name = state
                    .resolve(&target)
                    .ok_or_else(|| Fault::no_object("collection"))?;
                state.set_label(&name, &label)
            })
            .await
            .map_err(Fault::into_fdo)
    }

    #[zbus(property)]
    async fn locked(&self) -> bool {
        self.shared
            .read(|state| match self.name(state) {
                Ok(name) => state.is_locked(&name),
                Err(_) => true,
            })
            .await
    }

    #[zbus(property)]
    async fn created(&self) -> u64 {
        self.look(State::created).await
    }

    #[zbus(property)]
    async fn modified(&self) -> u64 {
        self.look(State::modified).await
    }

    #[zbus(signal)]
    pub(crate) async fn item_created(
        emitter: &SignalEmitter<'_>,
        item: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn item_deleted(
        emitter: &SignalEmitter<'_>,
        item: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn item_changed(
        emitter: &SignalEmitter<'_>,
        item: ObjectPath<'_>,
    ) -> zbus::Result<()>;
}
