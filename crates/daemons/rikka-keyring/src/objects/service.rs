use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rikka_secrets_protocol::names::COLLECTION_LABEL;
use rikka_secrets_protocol::{Algorithm, WireSecret};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use crate::fault::Fault;
use crate::objects::caller;
use crate::paths;
use crate::shared::Shared;

pub(crate) struct ServiceObject {
    shared: Arc<Shared>,
}

impl ServiceObject {
    pub(crate) fn new(shared: Arc<Shared>) -> ServiceObject {
        ServiceObject { shared }
    }
}

#[zbus::interface(name = "org.freedesktop.Secret.Service")]
impl ServiceObject {
    #[zbus(out_args("output", "result"))]
    async fn open_session(
        &self,
        algorithm: String,
        input: OwnedValue,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(OwnedValue, OwnedObjectPath), Fault> {
        let caller = caller(&header)?;
        let algorithm = Algorithm::from_name(&algorithm)
            .ok_or_else(|| Fault::unsupported(format!("algorithm {algorithm} is not supported")))?;
        let peer = match algorithm {
            Algorithm::Plain => None,
            Algorithm::Dh => Some(
                Vec::<u8>::try_from(input)
                    .map_err(|_| Fault::invalid("the DH input is not a byte array"))?,
            ),
        };
        let opened = self
            .shared
            .run(move |state| state.open_session(&caller, algorithm, peer.as_deref()))
            .await?;
        let output = match opened.public_key {
            Some(public_key) => Value::from(public_key),
            None => Value::from(""),
        };
        let output = output
            .try_to_owned()
            .map_err(|error| Fault::failed(error.to_string()))?;
        Ok((output, opened.path))
    }

    #[zbus(out_args("collection", "prompt"))]
    async fn create_collection(
        &self,
        properties: HashMap<String, OwnedValue>,
        alias: String,
    ) -> Result<(OwnedObjectPath, OwnedObjectPath), Fault> {
        let label = match properties.get(COLLECTION_LABEL) {
            Some(value) => String::try_from(value.clone())
                .map_err(|_| Fault::invalid("the collection label is not a string"))?,
            None => String::new(),
        };
        let collection = self
            .shared
            .run(move |state| state.create_collection(&label, &alias))
            .await?;
        Ok((collection, paths::none()))
    }

    #[zbus(out_args("unlocked", "locked"))]
    async fn search_items(
        &self,
        attributes: HashMap<String, String>,
    ) -> (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) {
        let query: BTreeMap<String, String> = attributes.into_iter().collect();
        let unlocked = self.shared.run(move |state| state.search(&query)).await;
        (unlocked, Vec::new())
    }

    #[zbus(out_args("unlocked", "prompt"))]
    async fn unlock(
        &self,
        objects: Vec<OwnedObjectPath>,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(Vec<OwnedObjectPath>, OwnedObjectPath), Fault> {
        let caller = caller(&header)?;
        let unlocking = self
            .shared
            .run(move |state| state.unlock(objects, &caller))
            .await;
        Ok((unlocking.unlocked, unlocking.prompt))
    }

    #[zbus(out_args("locked", "Prompt"))]
    async fn lock(&self, objects: Vec<OwnedObjectPath>) -> (Vec<OwnedObjectPath>, OwnedObjectPath) {
        let locked = self.shared.run(move |state| state.lock(objects)).await;
        (locked, paths::none())
    }

    async fn lock_service(&self) {
        self.shared.run(|state| state.lock_all()).await;
    }

    #[zbus(out_args("secrets"))]
    async fn get_secrets(
        &self,
        items: Vec<OwnedObjectPath>,
        session: OwnedObjectPath,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<HashMap<OwnedObjectPath, WireSecret>, Fault> {
        let caller = caller(&header)?;
        let found = self
            .shared
            .run(move |state| state.secrets(&items, &session, &caller))
            .await?;
        Ok(found.into_iter().collect())
    }

    #[zbus(out_args("collection"))]
    async fn read_alias(&self, name: String) -> OwnedObjectPath {
        self.shared
            .read(|state| {
                state
                    .resolve_alias(&name)
                    .map_or_else(paths::none, |collection| paths::collection(&collection))
            })
            .await
    }

    async fn set_alias(&self, name: String, collection: OwnedObjectPath) -> Result<(), Fault> {
        self.shared
            .run(move |state| state.set_alias(&name, &collection))
            .await
    }

    #[zbus(property)]
    async fn collections(&self) -> Vec<OwnedObjectPath> {
        self.shared.read(|state| state.collection_paths()).await
    }

    #[zbus(signal)]
    pub(crate) async fn collection_created(
        emitter: &SignalEmitter<'_>,
        collection: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn collection_deleted(
        emitter: &SignalEmitter<'_>,
        collection: ObjectPath<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn collection_changed(
        emitter: &SignalEmitter<'_>,
        collection: ObjectPath<'_>,
    ) -> zbus::Result<()>;
}
