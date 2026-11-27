use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use prost::Message;

use crate::client_tag::client_tag_hash;
use crate::encryption::{Encryption, Lock, PassphraseType};
use crate::error::{Error, Result};
use crate::local::{LocalEntity, LocalState};
use crate::proto::{
    ChipBag, ClientConfigParams, ClientStatus, ClientToServerMessage, ClientToServerResponse,
    CommitMessage, CommitResponse, DataTypeProgressMarker, ENCRYPTED_NAME, EntitySpecifics,
    GetUpdateTriggers, GetUpdatesCallerInfo, GetUpdatesMessage, GetUpdatesResponse, NIGORI,
    NIGORI_NAME, NIGORI_TAG, Notes, PASSWORDS, PROTOCOL_VERSION, PasswordSpecifics,
    PasswordSpecificsData, PasswordSpecificsMetadata, SyncEntity, commit_result, contents,
    error_type, origin,
};
use crate::wire;

const UNCOMMITTED: i64 = -1;
const DEFAULT_POLL: Duration = Duration::from_secs(8 * 60 * 60);
const DEFAULT_COMMIT_BATCH: usize = 25;
const DEFAULT_THROTTLE: Duration = Duration::from_secs(2 * 60 * 60);
const KNOWN_PASSWORD_FIELDS: [std::ops::RangeInclusive<u32>; 2] = [1..=20, 22..=29];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    Periodic,
    Trigger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entity {
    server_id: String,
    version: i64,
    ctime: i64,
    mtime: i64,
    data: PasswordSpecificsData,
    unknown: Vec<u8>,
    deleted: bool,
    sequence: u64,
    acked: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Flight {
    Updates,
    Commit {
        nigori: bool,
        entries: Vec<(String, u64)>,
    },
}

#[derive(Debug)]
pub struct Engine {
    account: String,
    cache_guid: String,
    birthday: Option<String>,
    bag_of_chips: Option<Vec<u8>>,
    nigori_marker: Option<Vec<u8>>,
    passwords_marker: Option<Vec<u8>>,
    encryption: Encryption,
    nigori_id: String,
    nigori_version: i64,
    nigori_unsynced: bool,
    entities: BTreeMap<String, Entity>,
    stashed: HashMap<String, SyncEntity>,
    poll_interval: Duration,
    commit_batch: usize,
    flight: Option<Flight>,
    refresh: Option<Refresh>,
    resolving_conflict: bool,
    merging: bool,
    revision: u64,
}

impl Entity {
    fn is_unsynced(&self) -> bool {
        self.sequence > self.acked
    }

    fn encoded(&self) -> Vec<u8> {
        let mut encoded = self.data.encode_to_vec();
        encoded.extend_from_slice(&self.unknown);
        encoded
    }
}

impl Engine {
    #[must_use]
    pub fn new(account: &str) -> Engine {
        Engine {
            account: account.to_owned(),
            cache_guid: wire::cache_guid(),
            birthday: None,
            bag_of_chips: None,
            nigori_marker: None,
            passwords_marker: None,
            encryption: Encryption::default(),
            nigori_id: String::new(),
            nigori_version: 0,
            nigori_unsynced: false,
            entities: BTreeMap::new(),
            stashed: HashMap::new(),
            poll_interval: DEFAULT_POLL,
            commit_batch: DEFAULT_COMMIT_BATCH,
            flight: None,
            refresh: None,
            resolving_conflict: false,
            merging: true,
            revision: 0,
        }
    }

    pub fn restore(account: &str, snapshot: &[u8]) -> Result<Engine> {
        let local = LocalState::decode(snapshot)?;
        if local.cache_guid.is_empty() {
            return Err(Error::Corrupt("snapshot without a cache guid"));
        }
        let mut entities = BTreeMap::new();
        for entity in local.entities {
            let data = PasswordSpecificsData::decode(entity.data.as_slice())?;
            let unknown = wire::unknown_fields(&entity.data, is_known_password_field);
            entities.insert(
                entity.tag,
                Entity {
                    server_id: entity.server_id,
                    version: entity.version,
                    ctime: entity.ctime,
                    mtime: entity.mtime,
                    data,
                    unknown,
                    deleted: entity.deleted,
                    sequence: entity.sequence,
                    acked: entity.acked,
                },
            );
        }
        let stashed = local
            .stashed
            .into_iter()
            .filter_map(|entity| Some((entity.client_tag_hash.clone()?, entity)))
            .collect();
        let merging = local.nigori_marker.is_none() || local.passwords_marker.is_none();
        Ok(Engine {
            account: account.to_owned(),
            cache_guid: local.cache_guid,
            birthday: local.birthday,
            bag_of_chips: local.bag_of_chips,
            nigori_marker: local.nigori_marker,
            passwords_marker: local.passwords_marker,
            encryption: local.nigori.map(Encryption::from_local).unwrap_or_default(),
            nigori_id: local.nigori_id,
            nigori_version: local.nigori_version,
            nigori_unsynced: local.nigori_unsynced,
            entities,
            stashed,
            poll_interval: match local.poll_interval {
                0 => DEFAULT_POLL,
                seconds => Duration::from_secs(u64::from(seconds)),
            },
            commit_batch: match local.commit_batch {
                0 => DEFAULT_COMMIT_BATCH,
                batch => batch as usize,
            },
            flight: None,
            refresh: None,
            resolving_conflict: false,
            merging,
            revision: 0,
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        LocalState {
            cache_guid: self.cache_guid.clone(),
            birthday: self.birthday.clone(),
            bag_of_chips: self.bag_of_chips.clone(),
            nigori_marker: self.nigori_marker.clone(),
            passwords_marker: self.passwords_marker.clone(),
            nigori: Some(self.encryption.to_local()),
            entities: self
                .entities
                .iter()
                .map(|(tag, entity)| LocalEntity {
                    tag: tag.clone(),
                    server_id: entity.server_id.clone(),
                    version: entity.version,
                    ctime: entity.ctime,
                    mtime: entity.mtime,
                    data: entity.encoded(),
                    deleted: entity.deleted,
                    sequence: entity.sequence,
                    acked: entity.acked,
                })
                .collect(),
            stashed: self.stashed.values().cloned().collect(),
            nigori_id: self.nigori_id.clone(),
            nigori_version: self.nigori_version,
            nigori_unsynced: self.nigori_unsynced,
            poll_interval: u32::try_from(self.poll_interval.as_secs()).unwrap_or(u32::MAX),
            commit_batch: u32::try_from(self.commit_batch).unwrap_or(u32::MAX),
        }
        .encode_to_vec()
    }

    #[must_use]
    pub fn cache_guid(&self) -> &str {
        &self.cache_guid
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn poll_interval(&self) -> Duration {
        self.poll_interval
    }

    #[must_use]
    pub fn lock(&self) -> Lock {
        self.encryption.lock()
    }

    #[must_use]
    pub fn passphrase_type(&self) -> Option<PassphraseType> {
        self.encryption.passphrase_type()
    }

    #[must_use]
    pub fn has_synced(&self) -> bool {
        self.nigori_marker.is_some() && self.passwords_marker.is_some()
    }

    #[must_use]
    pub fn unsynced(&self) -> usize {
        self.entities
            .values()
            .filter(|entity| entity.is_unsynced())
            .count()
    }

    #[must_use]
    pub fn undecryptable(&self) -> usize {
        self.stashed.len()
    }

    pub fn passwords(&self) -> impl Iterator<Item = (&str, &PasswordSpecificsData)> {
        self.entities
            .iter()
            .filter(|(_, entity)| !entity.deleted)
            .map(|(tag, entity)| (tag.as_str(), &entity.data))
    }

    #[must_use]
    pub fn password(&self, tag: &str) -> Option<&PasswordSpecificsData> {
        self.entities
            .get(tag)
            .filter(|entity| !entity.deleted)
            .map(|entity| &entity.data)
    }

    pub fn request_refresh(&mut self, refresh: Refresh) {
        if self.refresh != Some(Refresh::Trigger) {
            self.refresh = Some(refresh);
        }
    }

    pub fn offer_passphrase(&mut self, passphrase: &str) -> Result<bool> {
        let accepted = self.encryption.offer_passphrase(passphrase)?;
        if accepted {
            self.unlocked();
        }
        Ok(accepted)
    }

    pub fn offer_vault_keys(&mut self, keys: &[Vec<u8>]) -> Result<bool> {
        let accepted = self.encryption.offer_vault_keys(keys)?;
        if accepted {
            self.unlocked();
        }
        Ok(accepted)
    }

    pub fn put(&mut self, data: PasswordSpecificsData, replacing: Option<&str>) -> String {
        let tag = client_tag_hash(&data);
        let now = wire::now_ms();
        let mut unknown = Vec::new();
        if let Some(previous) = replacing.filter(|previous| *previous != tag) {
            if let Some(replaced) = self.entities.get(previous) {
                unknown.clone_from(&replaced.unknown);
            }
            self.remove(previous);
        }
        match self.entities.get_mut(&tag) {
            Some(entity) => {
                entity.data = data;
                entity.deleted = false;
                entity.mtime = now;
                entity.sequence += 1;
            }
            None => {
                self.entities.insert(
                    tag.clone(),
                    Entity {
                        server_id: String::new(),
                        version: UNCOMMITTED,
                        ctime: now,
                        mtime: now,
                        data,
                        unknown,
                        deleted: false,
                        sequence: 1,
                        acked: 0,
                    },
                );
            }
        }
        self.revision += 1;
        tag
    }

    pub fn remove(&mut self, tag: &str) {
        let Some(entity) = self.entities.get_mut(tag) else {
            return;
        };
        let in_flight = matches!(
            &self.flight,
            Some(Flight::Commit { entries, .. }) if entries.iter().any(|(sent, _)| sent == tag)
        );
        if entity.version == UNCOMMITTED && !in_flight {
            self.entities.remove(tag);
        } else if !entity.deleted {
            entity.deleted = true;
            entity.mtime = wire::now_ms();
            entity.sequence += 1;
        }
        self.revision += 1;
    }

    pub fn next_request(&mut self) -> Option<ClientToServerMessage> {
        if self.flight.is_some() {
            return None;
        }
        if !self.has_synced() || self.refresh.is_some() {
            self.flight = Some(Flight::Updates);
            return Some(self.updates_request());
        }
        if !self.encryption.is_initialized() && !self.nigori_id.is_empty() {
            let initialized = self.encryption.initialize_with_keystore(wire::now_ms());
            if initialized.is_some() {
                self.nigori_unsynced = true;
                self.unlocked();
            }
        }
        if self.nigori_unsynced {
            return self.nigori_commit();
        }
        self.passwords_commit()
    }

    pub fn abandon(&mut self) {
        self.flight = None;
    }

    pub fn receive(&mut self, response: ClientToServerResponse) -> Result<()> {
        let Some(flight) = self.flight.take() else {
            return Err(Error::Protocol("an answer nobody asked for"));
        };
        self.check(&response)?;
        if let Some(chips) = response.new_bag_of_chips {
            self.bag_of_chips = Some(chips.encode_to_vec());
        }
        if let Some(command) = &response.client_command {
            if let Some(seconds) = command
                .set_sync_poll_interval
                .filter(|seconds| *seconds > 0)
            {
                self.poll_interval = Duration::from_secs(seconds.unsigned_abs().into());
            }
            if let Some(batch) = command.max_commit_batch_size.filter(|batch| *batch > 0) {
                self.commit_batch = batch.unsigned_abs() as usize;
            }
        }
        match flight {
            Flight::Updates => {
                let updates = response
                    .get_updates
                    .ok_or(Error::Protocol("no updates in the answer"))?;
                self.updated(updates)
            }
            Flight::Commit { nigori, entries } => {
                let commit = response
                    .commit
                    .ok_or(Error::Protocol("no commit in the answer"))?;
                self.committed(nigori, &entries, &commit)
            }
        }
    }

    fn check(&mut self, response: &ClientToServerResponse) -> Result<()> {
        let error = response
            .error
            .as_ref()
            .and_then(|error| error.error_type)
            .or(response.error_code)
            .unwrap_or(error_type::SUCCESS);
        let birthday_changed = match (&self.birthday, &response.store_birthday) {
            (Some(known), Some(sent)) => known != sent,
            _ => false,
        };
        match error {
            error_type::SUCCESS if birthday_changed => {
                self.reset();
                Err(Error::StoreReset)
            }
            error_type::SUCCESS => {
                if self.birthday.is_none() {
                    self.birthday.clone_from(&response.store_birthday);
                }
                Ok(())
            }
            error_type::NOT_MY_BIRTHDAY
            | error_type::CLIENT_DATA_OBSOLETE
            | error_type::ENCRYPTION_OBSOLETE
            | error_type::MIGRATION_DONE => {
                self.reset();
                Err(Error::StoreReset)
            }
            error_type::THROTTLED => Err(Error::Throttled(
                response
                    .client_command
                    .as_ref()
                    .and_then(|command| command.throttle_delay_seconds)
                    .filter(|seconds| *seconds > 0)
                    .map_or(DEFAULT_THROTTLE, |seconds| {
                        Duration::from_secs(seconds.unsigned_abs().into())
                    }),
            )),
            error_type::DISABLED_BY_ADMIN => Err(Error::Disabled),
            error_type::TRANSIENT_ERROR | error_type::PARTIAL_FAILURE => Err(Error::Transient),
            _ => Err(Error::Refused(
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.error_description.clone())
                    .or_else(|| response.error_message.clone())
                    .unwrap_or_default(),
            )),
        }
    }

    fn reset(&mut self) {
        self.cache_guid = wire::cache_guid();
        self.birthday = None;
        self.bag_of_chips = None;
        self.nigori_marker = None;
        self.passwords_marker = None;
        self.encryption.clear();
        self.nigori_id.clear();
        self.nigori_version = 0;
        self.nigori_unsynced = false;
        self.stashed.clear();
        self.entities
            .retain(|_, entity| entity.is_unsynced() && !entity.deleted);
        for entity in self.entities.values_mut() {
            entity.server_id.clear();
            entity.version = UNCOMMITTED;
            entity.sequence = 1;
            entity.acked = 0;
        }
        self.refresh = None;
        self.resolving_conflict = false;
        self.merging = true;
        self.revision += 1;
    }

    fn updates_request(&self) -> ClientToServerMessage {
        let initial = self.merging;
        let origin = match (initial, self.refresh) {
            (true, _) => origin::NEW_CLIENT,
            (false, Some(Refresh::Periodic)) => origin::PERIODIC,
            (false, _) => origin::GU_TRIGGER,
        };
        let triggers = (origin == origin::GU_TRIGGER).then(|| GetUpdateTriggers {
            notification_hint: Vec::new(),
            client_dropped_hints: Some(false),
            invalidations_out_of_sync: Some(true),
            local_modification_nudges: Some(0),
            datatype_refresh_nudges: Some(1),
            server_dropped_hints: Some(false),
            initial_sync_in_progress: Some(false),
            sync_for_resolve_conflict_in_progress: Some(self.resolving_conflict),
        });
        let marker = |data_type: i32, token: &Option<Vec<u8>>| DataTypeProgressMarker {
            data_type_id: Some(data_type),
            token: Some(token.clone().unwrap_or_default()),
            get_update_triggers: triggers.clone(),
            gc_directive: None,
        };
        self.message(
            contents::GET_UPDATES,
            None,
            Some(GetUpdatesMessage {
                caller_info: Some(GetUpdatesCallerInfo {
                    notifications_enabled: Some(false),
                }),
                fetch_folders: Some(true),
                from_progress_marker: vec![
                    marker(NIGORI, &self.nigori_marker),
                    marker(PASSWORDS, &self.passwords_marker),
                ],
                streaming: None,
                need_encryption_key: Some(self.encryption.needs_keystore_keys()),
                get_updates_origin: Some(origin),
                is_retry: None,
            }),
        )
    }

    fn message(
        &self,
        contents: i32,
        commit: Option<CommitMessage>,
        get_updates: Option<GetUpdatesMessage>,
    ) -> ClientToServerMessage {
        ClientToServerMessage {
            share: self.account.clone(),
            protocol_version: Some(PROTOCOL_VERSION),
            message_contents: contents,
            commit,
            get_updates,
            store_birthday: self.birthday.clone(),
            bag_of_chips: Some(
                self.bag_of_chips
                    .as_deref()
                    .and_then(|chips| ChipBag::decode(chips).ok())
                    .unwrap_or_default(),
            ),
            client_status: Some(ClientStatus {
                is_sync_feature_enabled: Some(true),
            }),
        }
    }

    fn commit_message(&self, entries: Vec<SyncEntity>) -> ClientToServerMessage {
        self.message(
            contents::COMMIT,
            Some(CommitMessage {
                entries,
                cache_guid: Some(self.cache_guid.clone()),
                config_params: Some(ClientConfigParams {
                    enabled_type_ids: vec![NIGORI, PASSWORDS],
                    cookie_jar_mismatch: Some(false),
                    single_client: Some(false),
                }),
                padding: Some(wire::padding()),
            }),
            None,
        )
    }

    fn nigori_commit(&mut self) -> Option<ClientToServerMessage> {
        let specifics = self.encryption.specifics()?;
        let now = wire::now_ms();
        let entity = SyncEntity {
            id_string: Some(self.nigori_id.clone()),
            version: Some(self.nigori_version),
            mtime: Some(now),
            ctime: Some(now),
            name: Some(NIGORI_NAME.to_owned()),
            deleted: Some(false),
            specifics: Some(EntitySpecifics {
                nigori: Some(specifics),
                ..EntitySpecifics::default()
            }),
            folder: Some(true),
            ..SyncEntity::default()
        };
        self.flight = Some(Flight::Commit {
            nigori: true,
            entries: Vec::new(),
        });
        Some(self.commit_message(vec![entity]))
    }

    fn passwords_commit(&mut self) -> Option<ClientToServerMessage> {
        if self.encryption.lock() != Lock::Open || self.encryption.primary().is_none() {
            return None;
        }
        let plain_metadata = !self.encryption.uses_explicit_passphrase();
        let mut sent = Vec::new();
        let mut entries = Vec::new();
        for (tag, entity) in &mut self.entities {
            if !entity.is_unsynced() {
                continue;
            }
            if entries.len() == self.commit_batch {
                break;
            }
            if entity.server_id.is_empty() {
                entity.server_id = wire::uuid();
            }
            let specifics = match entity.deleted {
                true => PasswordSpecifics::default(),
                false => {
                    let notes = entity.data.notes.clone().unwrap_or_default();
                    PasswordSpecifics {
                        encrypted: self.encryption.encrypt(&entity.encoded()),
                        unencrypted_metadata: plain_metadata.then(|| metadata(&entity.data)),
                        encrypted_notes_backup: self.encryption.encrypt(&notes.encode_to_vec()),
                    }
                }
            };
            entries.push(SyncEntity {
                id_string: Some(entity.server_id.clone()),
                version: Some(entity.version.max(0)),
                mtime: Some(entity.mtime),
                ctime: (!entity.deleted).then_some(entity.ctime),
                name: Some(match entity.deleted {
                    true => String::new(),
                    false => ENCRYPTED_NAME.to_owned(),
                }),
                deleted: Some(entity.deleted),
                specifics: Some(EntitySpecifics {
                    password: Some(specifics),
                    ..EntitySpecifics::default()
                }),
                client_tag_hash: Some(tag.clone()),
                ..SyncEntity::default()
            });
            sent.push((tag.clone(), entity.sequence));
        }
        if entries.is_empty() {
            return None;
        }
        self.flight = Some(Flight::Commit {
            nigori: false,
            entries: sent,
        });
        Some(self.commit_message(entries))
    }

    fn committed(
        &mut self,
        nigori: bool,
        sent: &[(String, u64)],
        commit: &CommitResponse,
    ) -> Result<()> {
        if nigori {
            let answer = commit
                .entryresponse
                .first()
                .ok_or(Error::Protocol("no answer to the nigori commit"))?;
            self.nigori_unsynced = false;
            return match answer.response_type.unwrap_or_default() {
                commit_result::SUCCESS => {
                    self.nigori_version = answer.version.unwrap_or(self.nigori_version);
                    Ok(())
                }
                commit_result::CONFLICT => {
                    self.conflicted();
                    Ok(())
                }
                commit_result::INVALID_MESSAGE => Err(Error::Protocol("nigori commit refused")),
                _ => {
                    self.nigori_unsynced = true;
                    Err(Error::Transient)
                }
            };
        }
        if commit.entryresponse.len() != sent.len() {
            return Err(Error::Protocol("commit answers do not match the entries"));
        }
        let mut outcome = Ok(());
        for ((tag, sequence), answer) in sent.iter().zip(&commit.entryresponse) {
            match answer.response_type.unwrap_or_default() {
                commit_result::SUCCESS => self.acknowledged(tag, *sequence, answer),
                commit_result::CONFLICT => self.conflicted(),
                commit_result::INVALID_MESSAGE => {
                    outcome = Err(Error::Protocol("commit entry refused"));
                }
                _ => outcome = outcome.and(Err(Error::Transient)),
            }
        }
        self.revision += 1;
        outcome
    }

    fn acknowledged(&mut self, tag: &str, sequence: u64, answer: &crate::proto::EntryResponse) {
        let Some(entity) = self.entities.get_mut(tag) else {
            return;
        };
        if let Some(id) = answer.id_string.as_ref().filter(|id| !id.is_empty()) {
            entity.server_id.clone_from(id);
        }
        entity.version = answer.version.unwrap_or(entity.version.max(0));
        entity.acked = sequence;
        if entity.deleted && !entity.is_unsynced() {
            self.entities.remove(tag);
        }
    }

    fn conflicted(&mut self) {
        self.refresh = Some(Refresh::Trigger);
        self.resolving_conflict = true;
    }

    fn updated(&mut self, updates: GetUpdatesResponse) -> Result<()> {
        self.encryption
            .set_keystore_keys(&updates.encryption_keys)?;
        let mut passwords = Vec::new();
        for entity in updates.entries {
            let is_nigori = entity.server_defined_unique_tag.as_deref() == Some(NIGORI_TAG)
                || entity
                    .specifics
                    .as_ref()
                    .is_some_and(|specifics| specifics.nigori.is_some());
            if is_nigori {
                self.nigori_updated(&entity)?;
            } else if entity
                .client_tag_hash
                .as_deref()
                .is_some_and(|tag| !tag.is_empty())
            {
                passwords.push(entity);
            }
        }
        for marker in updates.new_progress_marker {
            let token = marker.token.unwrap_or_default();
            match marker.data_type_id {
                Some(NIGORI) => self.nigori_marker = Some(token),
                Some(PASSWORDS) => {
                    self.passwords_marker = Some(token);
                    if let Some(watermark) = marker
                        .gc_directive
                        .and_then(|directive| directive.version_watermark)
                    {
                        self.entities.retain(|_, entity| {
                            entity.is_unsynced() || entity.version >= watermark
                        });
                    }
                }
                _ => {}
            }
        }
        for entity in passwords {
            self.password_updated(entity);
        }
        self.unstash();
        let finished = updates.changes_remaining.unwrap_or(0) == 0;
        if finished && !self.has_synced() {
            return Err(Error::Protocol("updates without progress markers"));
        }
        if finished {
            self.refresh = None;
            self.resolving_conflict = false;
            self.merging = false;
        } else if self.has_synced() {
            self.refresh = Some(self.refresh.unwrap_or(Refresh::Trigger));
        }
        self.revision += 1;
        Ok(())
    }

    fn nigori_updated(&mut self, entity: &SyncEntity) -> Result<()> {
        let version = entity.version.unwrap_or(0);
        if let Some(id) = entity.id_string.as_ref().filter(|id| !id.is_empty()) {
            self.nigori_id.clone_from(id);
        }
        if version <= self.nigori_version && self.encryption.is_initialized() {
            return Ok(());
        }
        self.nigori_version = version;
        let Some(specifics) = entity
            .specifics
            .as_ref()
            .and_then(|specifics| specifics.nigori.as_ref())
            .filter(|nigori| {
                nigori
                    .encryption_keybag
                    .as_ref()
                    .and_then(|keybag| keybag.blob.as_deref())
                    .is_some_and(|blob| !blob.is_empty())
            })
        else {
            return Ok(());
        };
        self.nigori_unsynced = false;
        let before = self.encryption.primary().map(str::to_owned);
        self.encryption.apply(specifics)?;
        if self.encryption.primary().map(str::to_owned) != before {
            self.unlocked();
        }
        Ok(())
    }

    fn unlocked(&mut self) {
        self.unstash();
        self.revision += 1;
    }

    fn unstash(&mut self) {
        if self.stashed.is_empty() {
            return;
        }
        let ready: Vec<String> = self
            .stashed
            .iter()
            .filter(|(_, entity)| {
                sealed(entity).is_some_and(|sealed| self.encryption.can_decrypt(sealed))
            })
            .map(|(tag, _)| tag.clone())
            .collect();
        for tag in ready {
            if let Some(entity) = self.stashed.remove(&tag) {
                self.password_updated(entity);
            }
        }
    }

    fn password_updated(&mut self, update: SyncEntity) {
        let Some(tag) = update.client_tag_hash.clone() else {
            return;
        };
        let version = update.version.unwrap_or(0);
        let known = self.entities.get(&tag);
        if known.is_some_and(|known| known.version >= version) {
            return;
        }
        if update.deleted == Some(true) {
            self.stashed.remove(&tag);
            match self.entities.get_mut(&tag) {
                Some(local) if local.is_unsynced() && !local.deleted => local.version = version,
                Some(_) => {
                    self.entities.remove(&tag);
                }
                None => {}
            }
            return;
        }
        let Some(sealed) = sealed(&update) else {
            return;
        };
        if !self.encryption.can_decrypt(sealed) {
            self.stashed.insert(tag, update);
            return;
        }
        let Some(remote) = self.opened(&update) else {
            tracing::warn!("skipping a password entity that does not decrypt");
            return;
        };
        self.stashed.remove(&tag);
        let merging = self.merging;
        if let Some(local) = self
            .entities
            .get_mut(&tag)
            .filter(|local| local.is_unsynced())
        {
            let local_wins = !local.deleted
                && merging
                && remote.data.date_created.unwrap_or(0) < local.data.date_created.unwrap_or(0);
            if local_wins {
                local.server_id = remote.server_id;
                local.version = remote.version;
                return;
            }
        }
        self.entities.insert(tag, remote);
    }

    fn opened(&self, update: &SyncEntity) -> Option<Entity> {
        let password = update.specifics.as_ref()?.password.as_ref()?;
        let sealed = password.encrypted.as_ref()?;
        let plain = self.encryption.decrypt(sealed)?;
        let mut data = PasswordSpecificsData::decode(plain.as_slice()).ok()?;
        if data.notes.is_none() {
            data.notes = password
                .encrypted_notes_backup
                .as_ref()
                .and_then(|backup| self.encryption.decrypt(backup))
                .and_then(|notes| Notes::decode(notes.as_slice()).ok())
                .filter(|notes| !notes.note.is_empty());
        }
        Some(Entity {
            server_id: update.id_string.clone().unwrap_or_default(),
            version: update.version.unwrap_or(0),
            ctime: update.ctime.unwrap_or(0),
            mtime: update.mtime.unwrap_or(0),
            data,
            unknown: wire::unknown_fields(&plain, is_known_password_field),
            deleted: false,
            sequence: 0,
            acked: 0,
        })
    }
}

fn sealed(entity: &SyncEntity) -> Option<&crate::proto::EncryptedData> {
    entity
        .specifics
        .as_ref()?
        .password
        .as_ref()?
        .encrypted
        .as_ref()
}

fn metadata(data: &PasswordSpecificsData) -> PasswordSpecificsMetadata {
    PasswordSpecificsMetadata {
        url: data.signon_realm.clone(),
        blacklisted: Some(data.blacklisted.unwrap_or(false)),
        date_last_used_windows_epoch_micros: Some(data.date_last_used.unwrap_or(0)),
        password_issues: data.password_issues.clone(),
        r#type: Some(data.r#type.unwrap_or(0)),
    }
}

fn is_known_password_field(field: u32) -> bool {
    KNOWN_PASSWORD_FIELDS
        .iter()
        .any(|known| known.contains(&field))
}
