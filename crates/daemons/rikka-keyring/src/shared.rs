use std::sync::{Arc, OnceLock};

use futures_util::lock::Mutex;
use rikka_secrets_protocol::names::SERVICE_PATH;
use zbus::Connection;
use zbus::object_server::{Interface, ObjectServer, SignalEmitter};
use zbus::zvariant::{OwnedObjectPath, Value};

use crate::objects::{
    CollectionObject, ItemObject, KeyringObject, PromptObject, ServiceObject, SessionObject,
};
use crate::paths::{self, Target};
use crate::state::{Change, State};

pub(crate) struct Shared {
    state: Arc<Mutex<State>>,
    connection: OnceLock<Connection>,
}

impl Shared {
    pub(crate) fn new(state: State) -> Arc<Shared> {
        Arc::new(Shared {
            state: Arc::new(Mutex::new(state)),
            connection: OnceLock::new(),
        })
    }

    pub(crate) fn attach(&self, connection: Connection) {
        if self.connection.set(connection).is_err() {
            tracing::warn!("keyring was attached to a second connection");
        }
    }

    pub(crate) async fn run<T: Send + 'static>(
        self: &Arc<Shared>,
        work: impl FnOnce(&mut State) -> T + Send + 'static,
    ) -> T {
        let mut guard = Arc::clone(&self.state).lock_owned().await;
        let (outcome, changes) = blocking::unblock(move || {
            guard.refresh();
            let outcome = work(&mut guard);
            (outcome, guard.take_changes())
        })
        .await;
        self.apply(changes).await;
        outcome
    }

    pub(crate) async fn read<T>(&self, look: impl FnOnce(&State) -> T) -> T {
        look(&*self.state.lock().await)
    }

    pub(crate) async fn publish_all(self: &Arc<Shared>) -> zbus::Result<()> {
        let (collections, aliases, items) = self
            .read(|state| {
                let collections = state.collection_names();
                let items: Vec<OwnedObjectPath> = collections
                    .iter()
                    .flat_map(|name| state.item_paths(name))
                    .collect();
                (collections, state.alias_names(), items)
            })
            .await;
        let server = self.connection()?.object_server();
        server
            .at(SERVICE_PATH, ServiceObject::new(Arc::clone(self)))
            .await?;
        server
            .at(SERVICE_PATH, KeyringObject::new(Arc::clone(self)))
            .await?;
        for name in collections {
            server
                .at(
                    paths::collection(&name),
                    CollectionObject::new(Arc::clone(self), Target::Collection(name)),
                )
                .await?;
        }
        for alias in aliases {
            server
                .at(
                    paths::alias(&alias),
                    CollectionObject::new(Arc::clone(self), Target::Alias(alias)),
                )
                .await?;
        }
        for path in items {
            self.register_item(&path).await?;
        }
        Ok(())
    }

    pub(crate) async fn client_left(self: &Arc<Shared>, name: String) {
        self.run(move |state| state.client_left(&name)).await;
    }

    fn connection(&self) -> zbus::Result<&Connection> {
        self.connection
            .get()
            .ok_or_else(|| zbus::Error::Failure("keyring is not attached to a bus".to_owned()))
    }

    async fn apply(self: &Arc<Shared>, changes: Vec<Change>) {
        for change in changes {
            if let Err(error) = self.apply_one(change).await {
                tracing::warn!(%error, "failed to publish keyring change");
            }
        }
    }

    async fn apply_one(self: &Arc<Shared>, change: Change) -> zbus::Result<()> {
        let connection = self.connection()?.clone();
        let server = connection.object_server();
        match change {
            Change::CollectionCreated(name) => {
                let path = paths::collection(&name);
                server
                    .at(
                        &path,
                        CollectionObject::new(Arc::clone(self), Target::Collection(name)),
                    )
                    .await?;
                ServiceObject::collection_created(
                    &SignalEmitter::new(&connection, SERVICE_PATH)?,
                    path.as_ref(),
                )
                .await?;
                self.service_changed().await?;
            }
            Change::CollectionDeleted { name, items } => {
                for id in items {
                    unpublish::<ItemObject>(server, &paths::item(&name, id)).await?;
                }
                let path = paths::collection(&name);
                unpublish::<CollectionObject>(server, &path).await?;
                ServiceObject::collection_deleted(
                    &SignalEmitter::new(&connection, SERVICE_PATH)?,
                    path.as_ref(),
                )
                .await?;
                self.service_changed().await?;
            }
            Change::CollectionChanged(name) => self.collection_changed(&name).await?,
            Change::Opened { collection, items } => {
                for id in items {
                    self.register_item(&paths::item(&collection, id)).await?;
                }
                self.collection_changed(&collection).await?;
            }
            Change::Closed { collection, items } => {
                for id in items {
                    unpublish::<ItemObject>(server, &paths::item(&collection, id)).await?;
                }
                self.collection_changed(&collection).await?;
            }
            Change::ItemCreated(collection, id) => {
                let path = paths::item(&collection, id);
                self.register_item(&path).await?;
                CollectionObject::item_created(
                    &SignalEmitter::new(&connection, paths::collection(&collection))?,
                    path.as_ref(),
                )
                .await?;
                self.collection_changed(&collection).await?;
            }
            Change::ItemDeleted(collection, id) => {
                let path = paths::item(&collection, id);
                unpublish::<ItemObject>(server, &path).await?;
                CollectionObject::item_deleted(
                    &SignalEmitter::new(&connection, paths::collection(&collection))?,
                    path.as_ref(),
                )
                .await?;
                self.collection_changed(&collection).await?;
            }
            Change::ItemChanged(collection, id) => {
                let path = paths::item(&collection, id);
                CollectionObject::item_changed(
                    &SignalEmitter::new(&connection, paths::collection(&collection))?,
                    path.as_ref(),
                )
                .await?;
                if let Ok(item) = server.interface::<_, ItemObject>(&path).await {
                    let emitter = item.signal_emitter();
                    let object = item.get().await;
                    object.label_changed(emitter).await?;
                    object.attributes_changed(emitter).await?;
                    object.modified_changed(emitter).await?;
                }
            }
            Change::SessionOpened(path) => {
                server
                    .at(&path, SessionObject::new(Arc::clone(self), path.clone()))
                    .await?;
            }
            Change::SessionClosed(path) => {
                unpublish::<SessionObject>(server, &path).await?;
            }
            Change::PromptOpened(path) => {
                server
                    .at(&path, PromptObject::new(Arc::clone(self), path.clone()))
                    .await?;
            }
            Change::PromptCompleted {
                path,
                dismissed,
                result,
            } => {
                PromptObject::completed(
                    &SignalEmitter::new(&connection, path.clone())?,
                    dismissed,
                    Value::from(result),
                )
                .await?;
                unpublish::<PromptObject>(server, &path).await?;
            }
            Change::AliasAdded(alias) => {
                server
                    .at(
                        paths::alias(&alias),
                        CollectionObject::new(Arc::clone(self), Target::Alias(alias)),
                    )
                    .await?;
            }
            Change::AliasRemoved(alias) => {
                unpublish::<CollectionObject>(server, &paths::alias(&alias)).await?;
            }
            Change::Keyring => {
                let keyring = server.interface::<_, KeyringObject>(SERVICE_PATH).await?;
                let emitter = keyring.signal_emitter();
                let object = keyring.get().await;
                object.state_changed(emitter).await?;
                object.auto_unlock_changed(emitter).await?;
                object.asking_changed(emitter).await?;
            }
        }
        Ok(())
    }

    async fn register_item(self: &Arc<Shared>, path: &OwnedObjectPath) -> zbus::Result<()> {
        let Some(Target::Item(collection, id)) = paths::target(path) else {
            return Ok(());
        };
        self.connection()?
            .object_server()
            .at(path, ItemObject::new(Arc::clone(self), collection, id))
            .await?;
        Ok(())
    }

    async fn service_changed(&self) -> zbus::Result<()> {
        let service = self
            .connection()?
            .object_server()
            .interface::<_, ServiceObject>(SERVICE_PATH)
            .await?;
        service
            .get()
            .await
            .collections_changed(service.signal_emitter())
            .await
    }

    async fn collection_changed(&self, name: &str) -> zbus::Result<()> {
        let connection = self.connection()?;
        let path = paths::collection(name);
        ServiceObject::collection_changed(
            &SignalEmitter::new(connection, SERVICE_PATH)?,
            path.as_ref(),
        )
        .await?;
        let Ok(collection) = connection
            .object_server()
            .interface::<_, CollectionObject>(&path)
            .await
        else {
            return Ok(());
        };
        let emitter = collection.signal_emitter();
        let object = collection.get().await;
        object.items_changed(emitter).await?;
        object.locked_changed(emitter).await?;
        object.label_changed(emitter).await?;
        object.modified_changed(emitter).await
    }
}

async fn unpublish<I: Interface>(
    server: &ObjectServer,
    path: &OwnedObjectPath,
) -> zbus::Result<()> {
    match server.remove::<I, _>(path).await {
        Ok(_) | Err(zbus::Error::InterfaceNotFound) => Ok(()),
        Err(error) => Err(error),
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Shared").finish_non_exhaustive()
    }
}
