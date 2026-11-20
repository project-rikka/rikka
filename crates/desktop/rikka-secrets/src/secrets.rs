use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use rikka_secrets_protocol::names::{
    BINARY_CONTENT, BUS_NAME, COLLECTION_INTERFACE, DEFAULT_ALIAS, ITEM_ATTRIBUTES, ITEM_INTERFACE,
    ITEM_LABEL, KEYRING_INTERFACE, NO_OBJECT, PROMPT_INTERFACE, SERVICE_INTERFACE, SERVICE_PATH,
    TEXT_CONTENT,
};
use rikka_secrets_protocol::{Algorithm, KeyExchange, Transport, WireSecret};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{Mutex, OnceCell};
use zbus::message::Type as MessageType;
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream, Proxy};
use zeroize::Zeroizing;

use crate::entry::{Entry, Found, ItemId, Secret, StoredItem};
use crate::error::{Error, Result};
use crate::keyring::Keyring;

const METHOD_TIMEOUT: Duration = Duration::from_secs(60);
const PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
const PROPERTIES_CHANGED: &str = "PropertiesChanged";
const COLLECTION_CHANGED: &str = "CollectionChanged";

static SHARED: OnceCell<Secrets> = OnceCell::const_new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Items,
    Keyring,
}

#[derive(Clone, Debug)]
pub struct Secrets {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    connection: Connection,
    session: Mutex<Option<Arc<Session>>>,
}

#[derive(Debug)]
pub(crate) struct Session {
    pub(crate) path: OwnedObjectPath,
    transport: Transport,
}

impl Session {
    pub(crate) fn seal(&self, plain: &[u8], content_type: &str) -> Result<WireSecret> {
        let sealed = self.transport.seal(plain)?;
        Ok(WireSecret {
            session: self.path.clone(),
            parameters: sealed.parameters,
            value: sealed.value,
            content_type: content_type.to_owned(),
        })
    }

    fn open(&self, secret: &WireSecret) -> Result<Zeroizing<Vec<u8>>> {
        Ok(self.transport.open(&secret.parameters, &secret.value)?)
    }
}

impl Secrets {
    pub async fn shared() -> Result<Secrets> {
        SHARED.get_or_try_init(Secrets::connect).await.cloned()
    }

    pub async fn unlocked() -> Result<Secrets> {
        let secrets = Secrets::shared().await?;
        secrets.unlock().await?;
        Ok(secrets)
    }

    pub async fn connect() -> Result<Secrets> {
        let connection = zbus::connection::Builder::session()
            .map_err(Error::NoBus)?
            .method_timeout(METHOD_TIMEOUT)
            .build()
            .await
            .map_err(Error::NoBus)?;
        Ok(Secrets {
            inner: Arc::new(Inner {
                connection,
                session: Mutex::new(None),
            }),
        })
    }

    #[must_use]
    pub fn keyring(&self) -> Keyring {
        Keyring::new(self.clone())
    }

    pub async fn unlock(&self) -> Result<()> {
        let collection = self.default_collection().await?;
        if !self.collection_locked(&collection).await? {
            return Ok(());
        }
        let (_, prompt): (Vec<OwnedObjectPath>, OwnedObjectPath) = self
            .service()
            .await?
            .call("Unlock", &(vec![collection],))
            .await
            .map_err(Error::from_call)?;
        if prompt.as_str() == NO_OBJECT {
            return Ok(());
        }
        let prompt = self.proxy(&prompt, PROMPT_INTERFACE).await?;
        let mut completed = prompt
            .receive_signal("Completed")
            .await
            .map_err(Error::from_call)?;
        prompt
            .call::<_, _, ()>("Prompt", &("",))
            .await
            .map_err(Error::from_call)?;
        let message = completed
            .next()
            .await
            .ok_or_else(|| Error::Service("the unlock prompt vanished".to_owned()))?;
        let (dismissed, _): (bool, OwnedValue) =
            message.body().deserialize().map_err(Error::from_call)?;
        match dismissed {
            true => Err(Error::Dismissed),
            false => Ok(()),
        }
    }

    pub async fn store(&self, entry: &Entry, label: &str, secret: &[u8]) -> Result<()> {
        self.store_typed(entry, label, secret, BINARY_CONTENT).await
    }

    pub async fn store_json<T: Serialize>(
        &self,
        entry: &Entry,
        label: &str,
        value: &T,
    ) -> Result<()> {
        let encoded = Zeroizing::new(serde_json::to_vec(value)?);
        self.store_typed(entry, label, &encoded, TEXT_CONTENT).await
    }

    pub async fn contains(&self, entry: &Entry) -> Result<bool> {
        Ok(!self.matching(entry).await?.is_empty())
    }

    pub async fn lookup(&self, entry: &Entry) -> Result<Option<Secret>> {
        let Some(first) = self.matching(entry).await?.into_iter().next() else {
            return Ok(None);
        };
        Ok(self
            .secrets(std::slice::from_ref(&first))
            .await?
            .remove(&first))
    }

    pub async fn lookup_json<T: DeserializeOwned>(&self, entry: &Entry) -> Result<Option<T>> {
        match self.lookup(entry).await? {
            Some(secret) => Ok(Some(serde_json::from_slice(secret.expose())?)),
            None => Ok(None),
        }
    }

    pub async fn search(&self, entry: &Entry) -> Result<Vec<Found>> {
        let items = self.matching(entry).await?;
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let mut secrets = self.secrets(&items).await?;
        let attributes = futures_util::future::try_join_all(items.iter().map(|item| {
            self.property::<HashMap<String, String>>(item, ITEM_INTERFACE, "Attributes")
        }))
        .await?;
        Ok(items
            .iter()
            .zip(attributes)
            .filter_map(|(item, attributes)| {
                Some(Found {
                    attributes: attributes.into_iter().collect(),
                    secret: secrets.remove(item)?,
                })
            })
            .collect())
    }

    pub async fn remove(&self, entry: &Entry) -> Result<usize> {
        let items = self.matching(entry).await?;
        for item in &items {
            self.delete_path(item).await?;
        }
        Ok(items.len())
    }

    pub async fn items(&self) -> Result<Vec<StoredItem>> {
        let collections: Vec<OwnedObjectPath> = self
            .property(&service_path(), SERVICE_INTERFACE, "Collections")
            .await?;
        let mut paths = Vec::new();
        for collection in collections {
            let items: Vec<OwnedObjectPath> = self
                .property(&collection, COLLECTION_INTERFACE, "Items")
                .await?;
            paths.extend(items);
        }
        futures_util::future::try_join_all(paths.into_iter().map(|path| self.stored_item(path)))
            .await
    }

    pub async fn delete(&self, item: &ItemId) -> Result<()> {
        self.delete_path(&item.0).await
    }

    pub async fn changes(&self) -> Result<impl Stream<Item = Change> + Send + 'static> {
        let items = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .sender(BUS_NAME)
            .and_then(|rule| rule.interface(COLLECTION_INTERFACE))
            .map_err(Error::from_call)?
            .build();
        let collections = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .sender(BUS_NAME)
            .and_then(|rule| rule.interface(SERVICE_INTERFACE))
            .and_then(|rule| rule.member(COLLECTION_CHANGED))
            .map_err(Error::from_call)?
            .build();
        let keyring = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .sender(BUS_NAME)
            .and_then(|rule| rule.interface(PROPERTIES_INTERFACE))
            .and_then(|rule| rule.member(PROPERTIES_CHANGED))
            .and_then(|rule| rule.path(SERVICE_PATH))
            .and_then(|rule| rule.arg(0, KEYRING_INTERFACE))
            .map_err(Error::from_call)?
            .build();
        let connection = &self.inner.connection;
        let streams = [
            (items, Change::Items),
            (collections, Change::Items),
            (keyring, Change::Keyring),
        ];
        let mut watched = Vec::with_capacity(streams.len());
        for (rule, change) in streams {
            let stream = MessageStream::for_match_rule(rule, connection, None)
                .await
                .map_err(Error::from_call)?;
            watched.push(
                stream
                    .filter_map(move |message| {
                        futures_util::future::ready(message.ok().map(|_| change))
                    })
                    .boxed(),
            );
        }
        Ok(futures_util::stream::select_all(watched))
    }

    pub(crate) async fn with_session<R, Call, Reply>(&self, call: Call) -> Result<(Arc<Session>, R)>
    where
        Call: Fn(Arc<Session>) -> Result<Reply>,
        Reply: Future<Output = zbus::Result<R>>,
    {
        let mut retried = false;
        loop {
            let session = self.session().await?;
            match call(Arc::clone(&session))?.await {
                Ok(reply) => return Ok((session, reply)),
                Err(error) if Error::is_lost_session(&error) && !retried => {
                    retried = true;
                    self.forget_session(&session).await;
                }
                Err(error) => return Err(Error::from_call(error)),
            }
        }
    }

    pub(crate) async fn proxy(
        &self,
        path: &OwnedObjectPath,
        interface: &'static str,
    ) -> Result<Proxy<'static>> {
        zbus::proxy::Builder::new(&self.inner.connection)
            .destination(BUS_NAME)
            .and_then(|builder| builder.path(path.clone()))
            .and_then(|builder| builder.interface(interface))
            .map_err(Error::from_call)?
            .cache_properties(CacheProperties::No)
            .build()
            .await
            .map_err(Error::from_call)
    }

    async fn session(&self) -> Result<Arc<Session>> {
        let mut held = self.inner.session.lock().await;
        if let Some(session) = held.as_ref() {
            return Ok(Arc::clone(session));
        }
        let exchange = KeyExchange::new()?;
        let (output, path): (OwnedValue, OwnedObjectPath) = self
            .service()
            .await?
            .call(
                "OpenSession",
                &(
                    Algorithm::Dh.name(),
                    Value::from(exchange.public_key().to_vec()),
                ),
            )
            .await
            .map_err(Error::from_call)?;
        let peer = Vec::<u8>::try_from(output)
            .map_err(|_| Error::Service("the session reply is not a public key".to_owned()))?;
        let session = Arc::new(Session {
            path,
            transport: Transport::Aes(exchange.agree(&peer)?),
        });
        *held = Some(Arc::clone(&session));
        Ok(session)
    }

    async fn forget_session(&self, stale: &Arc<Session>) {
        let mut held = self.inner.session.lock().await;
        if held
            .as_ref()
            .is_some_and(|session| Arc::ptr_eq(session, stale))
        {
            *held = None;
        }
    }

    async fn service(&self) -> Result<Proxy<'static>> {
        self.proxy(&service_path(), SERVICE_INTERFACE).await
    }

    async fn store_typed(
        &self,
        entry: &Entry,
        label: &str,
        secret: &[u8],
        content_type: &str,
    ) -> Result<()> {
        let collection = self.default_collection().await?;
        let attributes: HashMap<&str, &str> = entry
            .attributes()
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let properties = HashMap::from([
            (ITEM_LABEL, Value::from(label)),
            (ITEM_ATTRIBUTES, Value::from(attributes)),
        ]);
        let target = self.proxy(&collection, COLLECTION_INTERFACE).await?;
        self.with_session(|session| {
            let wire = session.seal(secret, content_type)?;
            let (target, properties) = (target.clone(), properties.clone());
            Ok(async move {
                target
                    .call::<_, _, (OwnedObjectPath, OwnedObjectPath)>(
                        "CreateItem",
                        &(&properties, &wire, true),
                    )
                    .await
            })
        })
        .await
        .map(|_| ())
    }

    async fn secrets(&self, items: &[OwnedObjectPath]) -> Result<HashMap<OwnedObjectPath, Secret>> {
        let service = self.service().await?;
        let (session, sealed) = self
            .with_session(|session| {
                let service = service.clone();
                Ok(async move {
                    service
                        .call::<_, _, HashMap<OwnedObjectPath, WireSecret>>(
                            "GetSecrets",
                            &(items, &session.path),
                        )
                        .await
                })
            })
            .await?;
        sealed
            .into_iter()
            .map(|(path, secret)| Ok((path, Secret::new(session.open(&secret)?))))
            .collect()
    }

    async fn matching(&self, entry: &Entry) -> Result<Vec<OwnedObjectPath>> {
        let collection = self.default_collection().await?;
        if self.collection_locked(&collection).await? {
            return Err(Error::Locked);
        }
        self.proxy(&collection, COLLECTION_INTERFACE)
            .await?
            .call("SearchItems", &(entry.attributes(),))
            .await
            .map_err(Error::from_call)
    }

    async fn stored_item(&self, path: OwnedObjectPath) -> Result<StoredItem> {
        let mut properties: HashMap<String, OwnedValue> = self
            .proxy(&path, PROPERTIES_INTERFACE)
            .await?
            .call("GetAll", &(ITEM_INTERFACE,))
            .await
            .map_err(Error::from_call)?;
        let mut take = |name: &str| {
            properties
                .remove(name)
                .ok_or_else(|| Error::Service(format!("an item has no {name}")))
        };
        let malformed = |name: &str| Error::Service(format!("an item has a malformed {name}"));
        let attributes = HashMap::<String, String>::try_from(take("Attributes")?)
            .map_err(|_| malformed("Attributes"))?;
        Ok(StoredItem {
            label: String::try_from(take("Label")?).map_err(|_| malformed("Label"))?,
            attributes: attributes.into_iter().collect(),
            created: u64::try_from(take("Created")?).map_err(|_| malformed("Created"))?,
            modified: u64::try_from(take("Modified")?).map_err(|_| malformed("Modified"))?,
            id: ItemId(path),
        })
    }

    async fn collection_locked(&self, collection: &OwnedObjectPath) -> Result<bool> {
        self.property(collection, COLLECTION_INTERFACE, "Locked")
            .await
    }

    async fn delete_path(&self, item: &OwnedObjectPath) -> Result<()> {
        self.proxy(item, ITEM_INTERFACE)
            .await?
            .call::<_, _, OwnedObjectPath>("Delete", &())
            .await
            .map_err(Error::from_call)?;
        Ok(())
    }

    async fn default_collection(&self) -> Result<OwnedObjectPath> {
        let path: OwnedObjectPath = self
            .service()
            .await?
            .call("ReadAlias", &(DEFAULT_ALIAS,))
            .await
            .map_err(Error::from_call)?;
        match path.as_str() == NO_OBJECT {
            true => Err(Error::NoCollection),
            false => Ok(path),
        }
    }

    async fn property<T>(
        &self,
        path: &OwnedObjectPath,
        interface: &'static str,
        name: &str,
    ) -> Result<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: Into<zbus::Error>,
    {
        self.proxy(path, interface)
            .await?
            .get_property(name)
            .await
            .map_err(Error::from_call)
    }
}

pub(crate) fn service_path() -> OwnedObjectPath {
    OwnedObjectPath::try_from(SERVICE_PATH).expect("the secret service path is a valid object path")
}
