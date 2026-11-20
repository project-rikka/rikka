use serde::Serialize;
use serde::de::DeserializeOwned;

use futures_util::{Stream, StreamExt};
use rikka_seam::net::SocketClient;
use rikka_seam::{CapabilityId, Delivery, Payload, Reply, SeamClient};
use rikka_types::Choice;

use crate::domain::Domain;
use crate::error::{Error, Result};
use crate::layers::{Layers, Source};
use crate::schema::Schema;
use crate::seam::protocol::{self, CAPABILITY, Changed};
use crate::store::Store;
use crate::value::{Value, Values};

#[derive(Debug)]
pub struct Settings {
    store: Store,
    seam: Option<SeamClient>,
    connection: Option<SocketClient>,
}

impl Settings {
    pub fn open(schema: Schema) -> Result<Self> {
        let Ok(connection) = SocketClient::from_env().inspect_err(|reason| {
            tracing::debug!("using local settings store: {reason}");
        }) else {
            return Self::local(schema);
        };
        let mut settings = Self::with_seam(schema, connection.client())?;
        settings.connection = Some(connection);
        Ok(settings)
    }

    pub fn with_seam(schema: Schema, seam: SeamClient) -> Result<Self> {
        Self::with_seam_and_layers(schema, seam, Layers::resolve()?)
    }

    pub fn with_seam_and_layers(schema: Schema, seam: SeamClient, layers: Layers) -> Result<Self> {
        let declared = seam.call(protocol::declare(&schema)).blocking_wait();
        if let Err(error) = declared {
            tracing::warn!("session rejected schema, using local settings store: {error}");
            return Self::local_with_layers(schema, layers);
        }
        Self::loaded(schema, Some(seam), layers)
    }

    pub fn mirror(schema: Schema, seam: SeamClient) -> Result<Self> {
        Self::loaded(schema, Some(seam), Layers::resolve()?)
    }

    pub fn adopt(domain: &str) -> Result<Self> {
        let connection = SocketClient::from_env().map_err(|reason| Error::Session {
            code: 0,
            message: format!("there is no session to ask about `{domain}`: {reason}"),
        })?;
        let mut settings = Self::adopt_with_seam(domain, connection.client())?;
        settings.connection = Some(connection);
        Ok(settings)
    }

    pub fn adopt_with_seam(domain: &str, seam: SeamClient) -> Result<Self> {
        Self::adopt_with_seam_and_layers(domain, seam, Layers::resolve()?)
    }

    pub fn adopt_with_seam_and_layers(
        domain: &str,
        seam: SeamClient,
        layers: Layers,
    ) -> Result<Self> {
        let domain: Domain = domain.parse()?;
        let described: Payload = seam
            .call(protocol::describe(domain.as_str()))
            .blocking_wait()
            .map_err(Error::from_seam)?;
        let schema: Schema = described.decode().map_err(Error::from_seam)?;
        Self::loaded(schema, Some(seam), layers)
    }

    pub fn local(schema: Schema) -> Result<Self> {
        Self::local_with_layers(schema, Layers::resolve()?)
    }

    pub fn local_with_layers(schema: Schema, layers: Layers) -> Result<Self> {
        Self::loaded(schema, None, layers)
    }

    fn loaded(schema: Schema, seam: Option<SeamClient>, layers: Layers) -> Result<Self> {
        let (store, report) = Store::load(schema, layers)?;
        report.log_warnings(store.domain());
        let mut settings = Self {
            store,
            seam,
            connection: None,
        };
        if settings.seam.is_some()
            && let Err(error) = settings.refresh()
        {
            tracing::warn!(
                "failed to read `{}` from session, using file contents: {error}",
                settings.domain(),
            );
        }
        Ok(settings)
    }

    #[must_use]
    pub fn domain(&self) -> &Domain {
        self.store.domain()
    }

    #[must_use]
    pub fn schema(&self) -> &Schema {
        self.store.schema()
    }

    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.seam.is_some()
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        self.store.get(key)
    }

    pub fn read<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.store.read(key)
    }

    pub fn read_or<T: DeserializeOwned>(&self, key: &str, declared: T) -> T {
        self.store.read_or(key, declared)
    }

    pub fn set_or_warn(&mut self, key: &str, value: &impl Serialize) {
        if let Err(error) = self.set(key, value) {
            tracing::warn!("failed to write `{}/{key}`: {error}", self.domain());
        }
    }

    pub fn choice<C: Choice>(&self, key: &str) -> C {
        self.store.choice(key)
    }

    pub fn value(&self, key: &str) -> Result<&Value> {
        self.store.value(key)
    }

    pub fn source(&self, key: &str) -> Result<Source> {
        self.store.source(key)
    }

    pub fn set(&mut self, key: &str, value: impl Serialize) -> Result<()> {
        self.set_value(key, Value::from(&value)?)
    }

    pub fn set_choice<C: Choice>(&mut self, key: &str, choice: C) -> Result<()> {
        self.set_value(key, Value::String(choice.slug().to_owned()))
    }

    pub fn set_value(&mut self, key: &str, value: Value) -> Result<()> {
        let Some(seam) = &self.seam else {
            return self.store.set_value(key, value);
        };
        self.apply_reply(seam.call(protocol::set(self.store.domain().as_str(), key, value)))
    }

    pub fn reset(&mut self, key: &str) -> Result<()> {
        let Some(seam) = &self.seam else {
            return self.store.reset(key);
        };
        self.apply_reply(seam.call(protocol::reset(self.store.domain().as_str(), key)))
    }

    fn apply_reply(&mut self, reply: Reply) -> Result<()> {
        match reply.blocking_wait() {
            Ok(answer) => {
                let values: Values = answer.decode().map_err(Error::from_seam)?;
                self.set_user_values(values);
                Ok(())
            }
            Err(error) => {
                let error = Error::from_seam(error);
                if !matches!(error, Error::Refused { .. })
                    && let Err(asking) = self.refresh()
                {
                    tracing::warn!(
                        "failed to refresh `{}` from session: {asking}",
                        self.domain(),
                    );
                }
                Err(error)
            }
        }
    }

    pub fn apply_change(&mut self, changed: &Changed) -> bool {
        if changed.domain != *self.store.domain() {
            return false;
        }
        self.set_user_values(changed.user_values.clone());
        true
    }

    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    #[must_use]
    pub fn user_values(&self) -> &Values {
        self.store.user_values()
    }

    pub fn set_user_values(&mut self, values: Values) {
        let report = self.store.set_user_values(values);
        report.log_warnings(self.store.domain());
    }

    #[must_use]
    pub fn changes(&self) -> Option<impl Stream<Item = Announcement> + Send + 'static> {
        let seam = self.seam.as_ref()?;
        let domain = self.store.domain().as_str().to_owned();
        let capability = CapabilityId::from(CAPABILITY);
        Some(
            seam.subscribe_to(CAPABILITY)
                .filter_map(move |delivery| {
                    announcement(delivery, domain.clone(), capability.clone())
                })
                .boxed(),
        )
    }

    pub fn refresh(&mut self) -> Result<()> {
        let Some(seam) = &self.seam else {
            let report = self.store.reload()?;
            report.log_warnings(self.store.domain());
            return Ok(());
        };
        let values: Values = seam
            .call(protocol::user_values(self.store.domain().as_str()))
            .blocking_wait()
            .map_err(Error::from_seam)?
            .decode()
            .map_err(Error::from_seam)?;
        self.set_user_values(values);
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Announcement {
    Changed(Changed),
    Lagged,
}

async fn announcement(
    delivery: Delivery,
    domain: String,
    capability: CapabilityId,
) -> Option<Announcement> {
    match delivery {
        Delivery::Event(event) => protocol::changed(&event, &domain).map(Announcement::Changed),
        Delivery::Lagged(lagged) => lagged.includes(&capability).then_some(Announcement::Lagged),
    }
}
