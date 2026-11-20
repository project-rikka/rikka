use rikka_sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use rikka_proton_account::{Account, AccountKeys, UserKeys};
use rikka_proton_api::{Api, Page, Paging, offload, parallel_map};
use rikka_proton_cache::Wired;
use rikka_proton_pgp::Pgp;

use crate::api::entry::{
    CreateEntriesRequest, CreateEntryRequest, DeleteEntriesRequest, EntriesPage, EntryPayload,
    OrderEntriesRequest, OrderResult, UpdateEntriesRequest, UpdateEntryRequest,
};
use crate::api::key::KeysPage;
use crate::api::key::{CreateKeyRequest, KeyPayload};
use crate::api::{
    EntryUpdate, LocalEntry, RemoteEntry, SyncAction, SyncKey, SyncOrder, SyncOutcome, SyncPlan,
};
use crate::crypto::AuthenticatorKey;
use crate::entry::Entry;
use crate::error::{Error, Result};

const KEYS: &str = "authenticator/v1/key";
const ENTRIES: &str = "authenticator/v1/entry";
const ENTRIES_BULK: &str = "authenticator/v1/entry/bulk";
const ENTRIES_ORDER: &str = "authenticator/v1/entry/order";

const SERVICE: &str = "authenticator";
const CONTENT_FORMAT_VERSION: u8 = 1;
const BATCH: usize = 100;

#[derive(Clone, Debug)]
pub struct AuthenticatorClient {
    pub(crate) account: Account,
    keys: UserKeys,
    sync_keys: Arc<Mutex<HashMap<String, SyncKey>>>,
}

impl AuthenticatorClient {
    pub fn new(api: Api, keys: UserKeys) -> Result<AuthenticatorClient> {
        AuthenticatorClient::with_pgp(api, Pgp::new(), keys)
    }

    pub fn with_pgp(api: Api, pgp: Pgp, keys: UserKeys) -> Result<AuthenticatorClient> {
        AuthenticatorClient::with_account(&Account::with_pgp(api, pgp), keys)
    }

    pub fn for_account(account: &Account, keys: &Arc<AccountKeys>) -> Result<AuthenticatorClient> {
        AuthenticatorClient::with_account(account, keys.user_signing()?)
    }

    fn with_account(account: &Account, keys: UserKeys) -> Result<AuthenticatorClient> {
        let service = account.api().environment().with_prefix(SERVICE)?;
        let api = account.api().with_environment(service);
        let mut scoped = Account::with_pgp(api, *account.pgp());
        if let Some(cache) = account.cache() {
            scoped = scoped.with_cache(cache.clone());
        }

        Ok(AuthenticatorClient {
            account: scoped,
            keys,
            sync_keys: Arc::default(),
        })
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        self.account.api()
    }

    pub async fn keys(&self) -> Result<Vec<SyncKey>> {
        let response: KeysPage = self.api().get(KEYS).send_field("Keys").await?;
        self.cache_keys(&response.keys);

        self.decrypt_keys(into_values(response.keys)).await
    }

    pub async fn create_key(&self) -> Result<SyncKey> {
        let wrapped = AuthenticatorKey::generate().wrap(self.account.pgp(), &self.keys)?;
        let request = CreateKeyRequest { key: wrapped };
        let response: Wired<KeyPayload> = self
            .api()
            .post(KEYS)
            .json(&request)
            .send_field("Key")
            .await?;
        self.add_cached_key(&response);

        let pgp = *self.account.pgp();
        let keys = self.keys.clone();
        let payload = response.into_value();
        let created = offload(move || SyncKey::decrypt(payload, &pgp, &keys)).await?;
        self.insert_sync_keys(std::slice::from_ref(&created));
        Ok(created)
    }

    pub async fn key(&self) -> Result<SyncKey> {
        match self.keys().await?.into_iter().next() {
            Some(key) => Ok(key),
            None => self.create_key().await,
        }
    }

    pub async fn entries(&self, keys: &[SyncKey]) -> Result<Vec<RemoteEntry>> {
        let payloads = self.payloads().await?;
        let (entries, _) = decrypt_entries(payloads, keys).await;

        Ok(entries)
    }

    pub async fn create_entry(&self, key: &SyncKey, entry: &Entry) -> Result<RemoteEntry> {
        let request = create_request(key, entry)?;
        let response: Wired<EntryPayload> = self
            .api()
            .post(ENTRIES)
            .json(&request)
            .send_field("Entry")
            .await?;
        self.upsert_cached_entries(std::slice::from_ref(&response));

        decrypt_entry(&response, key)
    }

    pub async fn create_entries(
        &self,
        key: &SyncKey,
        entries: &[Entry],
    ) -> Result<Vec<RemoteEntry>> {
        let mut created = Vec::with_capacity(entries.len());

        for batch in entries.chunks(BATCH) {
            let request = CreateEntriesRequest {
                entries: batch
                    .iter()
                    .map(|entry| create_request(key, entry))
                    .collect::<Result<Vec<CreateEntryRequest>>>()?,
            };

            let entries: Vec<Wired<EntryPayload>> = self
                .api()
                .post(ENTRIES_BULK)
                .json(&request)
                .send_field_or_default("Entries")
                .await?;
            self.upsert_cached_entries(&entries);
            created.extend(decrypt_all(&entries, key)?);
        }

        Ok(created)
    }

    pub async fn update_entry(&self, key: &SyncKey, update: &EntryUpdate) -> Result<RemoteEntry> {
        self.update_entries(key, std::slice::from_ref(update))
            .await?
            .into_iter()
            .next()
            .ok_or(Error::MissingEntry)
    }

    pub async fn update_entries(
        &self,
        key: &SyncKey,
        updates: &[EntryUpdate],
    ) -> Result<Vec<RemoteEntry>> {
        let mut updated = Vec::with_capacity(updates.len());

        for batch in updates.chunks(BATCH) {
            let request = UpdateEntriesRequest {
                entries: batch
                    .iter()
                    .map(|update| update_request(key, update))
                    .collect::<Result<Vec<UpdateEntryRequest>>>()?,
            };

            let entries: Vec<Wired<EntryPayload>> = self
                .api()
                .put(ENTRIES_BULK)
                .json(&request)
                .send_field_or_default("Entries")
                .await?;
            self.upsert_cached_entries(&entries);
            updated.extend(decrypt_all(&entries, key)?);
        }

        Ok(updated)
    }

    pub async fn delete_entry(&self, remote_id: &str) -> Result<()> {
        self.delete_entries(&[remote_id.to_owned()]).await
    }

    pub async fn delete_entries(&self, remote_ids: &[String]) -> Result<()> {
        for batch in remote_ids.chunks(BATCH) {
            let request = DeleteEntriesRequest {
                entry_ids: batch.to_vec(),
            };

            self.api()
                .delete(ENTRIES_BULK)
                .json(&request)
                .send_unit()
                .await?;
        }
        self.remove_cached_entries(remote_ids);

        Ok(())
    }

    pub async fn reorder(
        &self,
        starting_position: u32,
        remote_ids: &[String],
    ) -> Result<Timestamp> {
        let request = OrderEntriesRequest {
            starting_position,
            entry_ids: remote_ids.to_vec(),
        };

        let response: OrderResult = self
            .api()
            .put(ENTRIES_ORDER)
            .json(&request)
            .send_field("Result")
            .await?;
        self.reorder_cached_entries(starting_position, remote_ids);

        Ok(response.modified_at)
    }

    pub async fn sync(&self, local: &[LocalEntry]) -> Result<SyncOutcome> {
        let (mut keys, payloads) = futures_util::try_join!(self.keys(), self.payloads())?;
        if keys.is_empty() {
            keys.push(self.create_key().await?);
        }
        let key = &keys[0];

        let (remote, undecryptable) = decrypt_entries(payloads, &keys).await;
        let remote_by_id: HashMap<&str, &RemoteEntry> = remote
            .iter()
            .map(|entry| (entry.remote_id.as_str(), entry))
            .collect();
        let local_by_id: HashMap<&str, &LocalEntry> = local
            .iter()
            .map(|entry| (entry.entry.id.as_str(), entry))
            .collect();

        let sync_order = SyncOrder::compute(&remote, local);
        let order = match sync_order.starting_position() {
            Some(start) => {
                let ordered_at = self.reorder(start, &sync_order.remote_ids()).await?;
                sync_order.placements_modified_at(ordered_at)
            }
            None => Vec::new(),
        };
        let resolve_position = |id: &str, fallback: Option<u32>| {
            sync_order
                .position_of(id)
                .or_else(|| local_by_id.get(id).map(|entry| entry.position))
                .or(fallback)
        };

        let mut entries = Vec::new();
        let mut removed = Vec::new();
        let mut created = Vec::new();
        let mut updated = Vec::new();
        let mut remote_deletions = Vec::new();

        for step in SyncPlan::compute(&remote, local).steps() {
            match step.action {
                SyncAction::Upsert => entries.extend(
                    step.remote_id
                        .as_deref()
                        .and_then(|id| remote_by_id.get(id))
                        .map(|remote_entry| RemoteEntry {
                            position: resolve_position(
                                &remote_entry.entry.id,
                                remote_entry.position,
                            ),
                            ..(*remote_entry).clone()
                        }),
                ),
                SyncAction::DeleteLocal => removed.push(step.entry.id.clone()),
                SyncAction::DeleteEverywhere => {
                    removed.push(step.entry.id.clone());
                    remote_deletions.extend(step.remote_id.clone());
                }
                SyncAction::Push => match (step.remote_id.clone(), step.revision) {
                    (Some(remote_id), Some(revision)) => {
                        updated.push(EntryUpdate::new(remote_id, revision, step.entry.clone()));
                    }
                    _ => created.push(step.entry.clone()),
                },
            }
        }
        created.sort_by_key(|entry| resolve_position(&entry.id, None));

        self.delete_entries(&remote_deletions).await?;
        let pushed = [
            self.create_entries(key, &created).await?,
            self.update_entries(key, &updated).await?,
        ];
        entries.extend(pushed.into_iter().flatten().map(|entry| RemoteEntry {
            position: resolve_position(&entry.entry.id, None),
            ..entry
        }));

        Ok(SyncOutcome {
            entries,
            removed,
            order,
            undecryptable,
        })
    }

    async fn payloads(&self) -> Result<Vec<EntryPayload>> {
        let wired: Vec<Wired<EntryPayload>> = self
            .api()
            .get(ENTRIES)
            .pages(Paging::since())
            .field("Entries")
            .collect(|page: EntriesPage| Page::new(page.entries).next(page.last_id))
            .await?;
        self.cache_entries(&wired);
        Ok(into_values(wired))
    }

    pub(crate) async fn decrypt_keys(&self, payloads: Vec<KeyPayload>) -> Result<Vec<SyncKey>> {
        let ids: Vec<String> = payloads.iter().map(|payload| payload.id.clone()).collect();
        let undecrypted: Vec<KeyPayload> = {
            let sync_keys = self.sync_keys.lock();
            payloads
                .into_iter()
                .filter(|payload| !sync_keys.contains_key(&payload.id))
                .collect()
        };

        if !undecrypted.is_empty() {
            let pgp = *self.account.pgp();
            let keys = self.keys.clone();
            let decrypted = offload(move || {
                undecrypted
                    .into_iter()
                    .filter_map(|payload| match SyncKey::decrypt(payload, &pgp, &keys) {
                        Ok(key) => Some(key),
                        Err(error) => {
                            tracing::warn!(%error, "failed to decrypt authenticator key");
                            None
                        }
                    })
                    .collect::<Vec<SyncKey>>()
            })
            .await;
            self.insert_sync_keys(&decrypted);
        }

        let sync_keys = self.sync_keys.lock();
        let usable: Vec<SyncKey> = ids
            .iter()
            .filter_map(|id| sync_keys.get(id).cloned())
            .collect();

        if !ids.is_empty() && usable.is_empty() {
            return Err(Error::NoUsableKey);
        }

        Ok(usable)
    }

    fn insert_sync_keys(&self, keys: &[SyncKey]) {
        let mut sync_keys = self.sync_keys.lock();
        for key in keys {
            sync_keys.insert(key.id().to_owned(), key.clone());
        }
    }
}

pub(crate) async fn decrypt_entries(
    payloads: Vec<EntryPayload>,
    keys: &[SyncKey],
) -> (Vec<RemoteEntry>, usize) {
    let keys = Arc::new(keys.to_vec());
    let indexed: Vec<(u32, EntryPayload)> = (0..).zip(payloads).collect();
    let results = parallel_map(indexed, move |(position, payload)| {
        let entry = RemoteEntry::decrypt(&payload, &keys, Some(position));
        if entry.is_none() {
            tracing::warn!(entry = payload.id, "failed to decrypt authenticator entry");
        }
        entry
    })
    .await;

    let total = results.len();
    let decrypted: Vec<RemoteEntry> = results.into_iter().flatten().collect();
    let undecryptable = total - decrypted.len();
    (decrypted, undecryptable)
}

fn create_request(key: &SyncKey, entry: &Entry) -> Result<CreateEntryRequest> {
    Ok(CreateEntryRequest {
        key_id: key.id().to_owned(),
        content: key.key().seal(entry)?,
        content_format_version: CONTENT_FORMAT_VERSION,
    })
}

fn update_request(key: &SyncKey, update: &EntryUpdate) -> Result<UpdateEntryRequest> {
    Ok(UpdateEntryRequest {
        key_id: key.id().to_owned(),
        entry_id: update.remote_id.clone(),
        content: key.key().seal(&update.entry)?,
        content_format_version: CONTENT_FORMAT_VERSION,
        last_revision: update.revision,
    })
}

fn decrypt_entry(payload: &EntryPayload, key: &SyncKey) -> Result<RemoteEntry> {
    RemoteEntry::decrypt(payload, std::slice::from_ref(key), None)
        .ok_or(Error::UndecryptableContent)
}

fn decrypt_all(payloads: &[Wired<EntryPayload>], key: &SyncKey) -> Result<Vec<RemoteEntry>> {
    payloads
        .iter()
        .map(|payload| decrypt_entry(payload, key))
        .collect()
}

pub(crate) fn into_values<T>(wired: Vec<Wired<T>>) -> Vec<T> {
    wired.into_iter().map(Wired::into_value).collect()
}
