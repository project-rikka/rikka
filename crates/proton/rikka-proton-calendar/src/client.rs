use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{StreamExt, TryStreamExt, stream};
use rikka_proton_account::{Account, AccountKeys};
use rikka_proton_api::Api;
use rikka_proton_contacts::{ContactsClient, KtActivation, MailDefaults};
use rikka_proton_key_transparency::KeyTransparency;
use rikka_proton_pgp::{Pgp, PublicKey};
use serde::Deserialize;

use crate::component::Component;
use crate::error::{Error, Result};
use crate::event::Event;

pub(crate) const CALENDAR_V1: &str = "calendar/v1";
pub(crate) const CALENDAR_V2: &str = "calendar/v2";
const VTIMEZONE_BATCH: usize = 10;
const CANONICAL_BATCH: usize = 100;
pub(crate) const MAX_CONCURRENT_LOOKUPS: usize = 8;

#[derive(Debug, Clone)]
pub struct CalendarClient {
    pub(crate) account: Account,
    keys: Arc<AccountKeys>,
    pub(crate) contacts: ContactsClient,
    prod_id: String,
    pub(crate) cache_writes: Arc<Mutex<()>>,
}

#[derive(Deserialize)]
struct CanonicalResponse {
    #[serde(rename = "CanonicalEmail", default)]
    canonical: Option<String>,
}

#[derive(Deserialize)]
struct CanonicalEntry {
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "Response")]
    response: CanonicalResponse,
}

impl CalendarClient {
    #[must_use]
    pub fn new(api: Api, keys: Arc<AccountKeys>) -> CalendarClient {
        CalendarClient::with_account(Account::new(api), keys)
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp, keys: Arc<AccountKeys>) -> CalendarClient {
        CalendarClient::with_account(Account::with_pgp(api, pgp), keys)
    }

    #[must_use]
    pub fn for_account(account: &Account, keys: &Arc<AccountKeys>) -> CalendarClient {
        CalendarClient::with_account(account.clone(), Arc::clone(keys))
    }

    fn with_account(account: Account, keys: Arc<AccountKeys>) -> CalendarClient {
        let version = account.api().app_version();
        let prod_id = format!(
            "-//Proton AG//{} {}//EN",
            version.client(),
            version.version()
        );
        CalendarClient {
            contacts: ContactsClient::for_account(&account, &keys),
            account,
            keys,
            prod_id,
            cache_writes: Arc::default(),
        }
    }

    #[must_use]
    pub fn with_prod_id(mut self, prod_id: impl Into<String>) -> CalendarClient {
        self.prod_id = prod_id.into();
        self
    }

    #[must_use]
    pub fn with_contacts(mut self, contacts: ContactsClient) -> CalendarClient {
        self.contacts = contacts;
        self
    }

    #[must_use]
    pub fn with_keys(mut self, keys: &Arc<AccountKeys>) -> CalendarClient {
        self.keys = Arc::clone(keys);
        self.contacts = self.contacts.with_keys(keys);
        self
    }

    #[must_use]
    pub fn with_key_transparency(
        mut self,
        transparency: KeyTransparency,
        activation: KtActivation,
    ) -> CalendarClient {
        self.contacts = self
            .contacts
            .with_key_transparency(transparency, activation);
        self
    }

    #[must_use]
    pub fn contacts(&self) -> &ContactsClient {
        &self.contacts
    }

    #[must_use]
    pub fn account(&self) -> &Account {
        &self.account
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        self.account.api()
    }

    #[must_use]
    pub fn pgp(&self) -> &Pgp {
        self.account.pgp()
    }

    #[must_use]
    pub fn keys(&self) -> &AccountKeys {
        &self.keys
    }

    pub(crate) fn shared_keys(&self) -> Arc<AccountKeys> {
        Arc::clone(&self.keys)
    }

    #[must_use]
    pub fn prod_id(&self) -> &str {
        &self.prod_id
    }

    pub async fn vtimezones(&self, tzids: &[String]) -> Result<Vec<Component>> {
        let mut unique: Vec<&String> = tzids
            .iter()
            .filter(|tzid| !tzid.eq_ignore_ascii_case("UTC"))
            .collect();
        unique.sort();
        unique.dedup();

        let batches: Vec<&[&String]> = unique.chunks(VTIMEZONE_BATCH).collect();
        let responses: Vec<HashMap<String, String>> = stream::iter(&batches)
            .map(|batch| {
                let query: Vec<(&str, &str)> = batch
                    .iter()
                    .map(|tzid| ("Timezones[]", tzid.as_str()))
                    .collect();
                self.api()
                    .get(format!("{CALENDAR_V1}/vtimezones"))
                    .query(&query)
                    .send_field_or_default("Timezones")
            })
            .buffered(MAX_CONCURRENT_LOOKUPS)
            .try_collect()
            .await?;

        let mut vtimezones = Vec::new();
        for (batch, response) in batches.iter().zip(&responses) {
            for tzid in *batch {
                if let Some(text) = response.get(tzid.as_str()) {
                    vtimezones.extend(
                        Component::parse_all(text)?
                            .into_iter()
                            .filter(|component| component.is("VTIMEZONE")),
                    );
                }
            }
        }
        Ok(vtimezones)
    }

    pub async fn canonical_emails(&self, emails: &[String]) -> Result<HashMap<String, String>> {
        let responses: Vec<Vec<CanonicalEntry>> = stream::iter(emails.chunks(CANONICAL_BATCH))
            .map(|batch| {
                let query: Vec<(&str, &str)> = batch
                    .iter()
                    .map(|email| ("Emails[]", email.as_str()))
                    .collect();
                self.api()
                    .get("core/v4/addresses/canonical")
                    .query(&query)
                    .send_field_or_default("Responses")
            })
            .buffered(MAX_CONCURRENT_LOOKUPS)
            .try_collect()
            .await?;
        let mut canonical = HashMap::new();
        for entry in responses.into_iter().flatten() {
            let resolved = entry
                .response
                .canonical
                .unwrap_or_else(|| entry.email.to_lowercase());
            canonical.insert(entry.email, resolved);
        }
        for email in emails {
            canonical
                .entry(email.clone())
                .or_insert_with(|| email.trim().to_lowercase());
        }
        Ok(canonical)
    }

    pub async fn author_keys(&self, email: &str) -> Result<Vec<PublicKey>> {
        if email.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(self.contacts.verifying_keys(email).await?.to_vec())
    }

    pub async fn encryption_key(&self, email: &str) -> Result<PublicKey> {
        let preferences = self
            .contacts
            .encryption_preferences(email, &MailDefaults::default())
            .await?;
        if let Some(refusal) = preferences.error {
            return Err(Error::RecipientRefused(email.to_owned(), refusal));
        }
        match (preferences.internal, preferences.send_key) {
            (true, Some(key)) => Ok(key),
            _ => Err(Error::NoInviteeKey(email.to_owned())),
        }
    }

    pub async fn proton_attendee_keys(&self, event: &Event) -> Result<Vec<(String, PublicKey)>> {
        let external_emails: Vec<String> = event
            .attendees()
            .iter()
            .map(|attendee| attendee.email())
            .filter(|email| self.keys.address_by_email(email).is_none())
            .collect();
        let keys: Vec<Option<(String, PublicKey)>> = stream::iter(external_emails)
            .map(|email| async move {
                let preferences = self
                    .contacts
                    .encryption_preferences(&email, &MailDefaults::default())
                    .await?;
                Ok::<_, Error>(
                    match (
                        preferences.internal,
                        &preferences.error,
                        preferences.send_key,
                    ) {
                        (true, None, Some(key)) => Some((email, key)),
                        _ => None,
                    },
                )
            })
            .buffered(MAX_CONCURRENT_LOOKUPS)
            .try_collect()
            .await?;
        Ok(keys.into_iter().flatten().collect())
    }
}
