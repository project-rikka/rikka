use std::collections::BTreeMap;

use google_nigori::{Derivation, Nigori};
use google_sync::proto::{
    ClientToServerMessage, ClientToServerResponse, CommitResponse, DataTypeProgressMarker,
    EncryptedData, EncryptionKeys, EntitySpecifics, EntryResponse, GetUpdatesResponse, NIGORI,
    NigoriKey, NigoriSpecifics, PASSWORDS, PasswordSpecifics, PasswordSpecificsData, SyncEntity,
};
use google_sync::{Engine, Error};
use prost::Message;

pub const ACCOUNT: &str = "rikka@example.com";
pub const KEYSTORE_KEY: &[u8] = b"keystore key of the test account";

const NIGORI_ID: &str = "nigori-root";
const SUCCESS: i32 = 1;
const CONFLICT: i32 = 2;
const NOT_MY_BIRTHDAY: i32 = 2;

#[derive(Debug)]
pub struct FakeServer {
    pub birthday: String,
    pub page: usize,
    pub commits: Vec<SyncEntity>,
    pub requests: usize,
    version: i64,
    entities: BTreeMap<String, SyncEntity>,
}

impl Default for FakeServer {
    fn default() -> FakeServer {
        FakeServer::new()
    }
}
impl FakeServer {
    pub fn new() -> FakeServer {
        let mut server = FakeServer {
            birthday: "birthday-1".to_owned(),
            page: usize::MAX,
            commits: Vec::new(),
            requests: 0,
            version: 0,
            entities: BTreeMap::new(),
        };
        server.store(SyncEntity {
            id_string: Some(NIGORI_ID.to_owned()),
            server_defined_unique_tag: Some("google_chrome_nigori".to_owned()),
            name: Some("Nigori".to_owned()),
            folder: Some(true),
            specifics: Some(EntitySpecifics {
                nigori: Some(NigoriSpecifics::default()),
                ..EntitySpecifics::default()
            }),
            ..SyncEntity::default()
        });
        server
    }

    pub fn with_nigori(nigori: NigoriSpecifics) -> FakeServer {
        let mut server = FakeServer::new();
        let mut root = server.entities[NIGORI_ID].clone();
        root.specifics = Some(EntitySpecifics {
            nigori: Some(nigori),
            ..EntitySpecifics::default()
        });
        server.store(root);
        server
    }

    pub fn start_over(&mut self) {
        *self = FakeServer {
            birthday: "birthday-2".to_owned(),
            ..FakeServer::new()
        };
    }

    pub fn store_password(&mut self, tag: &str, key: &Nigori, plain: &[u8]) -> String {
        let id = format!("server-{}", self.version + 1);
        self.store(SyncEntity {
            id_string: Some(id.clone()),
            client_tag_hash: Some(tag.to_owned()),
            name: Some("encrypted".to_owned()),
            specifics: Some(EntitySpecifics {
                password: Some(PasswordSpecifics {
                    encrypted: Some(sealed(key, plain)),
                    ..PasswordSpecifics::default()
                }),
                ..EntitySpecifics::default()
            }),
            ..SyncEntity::default()
        });
        id
    }

    pub fn stored(&self, tag: &str) -> Option<&SyncEntity> {
        self.entities
            .values()
            .find(|entity| entity.client_tag_hash.as_deref() == Some(tag))
    }

    pub fn handle(&mut self, message: &ClientToServerMessage) -> ClientToServerResponse {
        self.requests += 1;
        let foreign = message
            .store_birthday
            .as_ref()
            .is_some_and(|birthday| *birthday != self.birthday);
        if foreign {
            return ClientToServerResponse {
                error_code: Some(NOT_MY_BIRTHDAY),
                ..ClientToServerResponse::default()
            };
        }
        ClientToServerResponse {
            commit: message.commit.as_ref().map(|commit| CommitResponse {
                entryresponse: commit
                    .entries
                    .iter()
                    .map(|entry| self.commit(entry))
                    .collect(),
            }),
            get_updates: message.get_updates.as_ref().map(|updates| {
                let mut entries = Vec::new();
                let mut markers = Vec::new();
                let mut remaining = 0;
                for marker in &updates.from_progress_marker {
                    let data_type = marker.data_type_id.unwrap_or_default();
                    let since = token(marker.token.as_deref().unwrap_or_default());
                    let mut fresh: Vec<&SyncEntity> = self
                        .entities
                        .values()
                        .filter(|entity| kind(entity) == data_type)
                        .filter(|entity| entity.version.unwrap_or(0) > since)
                        .collect();
                    fresh.sort_by_key(|entity| entity.version);
                    let sent = fresh.len().min(self.page);
                    remaining += (fresh.len() - sent) as i64;
                    let reached = fresh[..sent]
                        .last()
                        .map_or(since, |entity| entity.version.unwrap_or(0));
                    entries.extend(fresh[..sent].iter().map(|entity| (*entity).clone()));
                    markers.push(DataTypeProgressMarker {
                        data_type_id: Some(data_type),
                        token: Some(reached.to_string().into_bytes()),
                        ..DataTypeProgressMarker::default()
                    });
                }
                GetUpdatesResponse {
                    entries,
                    changes_remaining: Some(remaining),
                    new_progress_marker: markers,
                    encryption_keys: match updates.need_encryption_key {
                        Some(true) => vec![KEYSTORE_KEY.to_vec()],
                        _ => Vec::new(),
                    },
                }
            }),
            error_code: Some(0),
            store_birthday: Some(self.birthday.clone()),
            ..ClientToServerResponse::default()
        }
    }

    fn commit(&mut self, entry: &SyncEntity) -> EntryResponse {
        self.commits.push(entry.clone());
        let id = entry.id_string.clone().unwrap_or_default();
        let known = self
            .entities
            .values()
            .find(|entity| {
                entity.id_string.as_deref() == Some(id.as_str())
                    || (entry.client_tag_hash.is_some()
                        && entity.client_tag_hash == entry.client_tag_hash)
            })
            .cloned();
        let based_on = entry.version.unwrap_or(0);
        if let Some(known) = &known
            && known.version.unwrap_or(0) != based_on
        {
            return EntryResponse {
                response_type: Some(CONFLICT),
                ..EntryResponse::default()
            };
        }
        let id = match known {
            Some(known) => known.id_string.unwrap_or_default(),
            None => format!("server-{}", self.version + 1),
        };
        let mut stored = entry.clone();
        stored.id_string = Some(id.clone());
        if stored.server_defined_unique_tag.is_none() && id == NIGORI_ID {
            stored.server_defined_unique_tag = Some("google_chrome_nigori".to_owned());
        }
        let version = self.store(stored);
        EntryResponse {
            response_type: Some(SUCCESS),
            id_string: Some(id),
            version: Some(version),
            error_message: None,
        }
    }

    fn store(&mut self, mut entity: SyncEntity) -> i64 {
        self.version += 1;
        entity.version = Some(self.version);
        self.entities
            .insert(entity.id_string.clone().unwrap_or_default(), entity);
        self.version
    }
}

pub fn sync(engine: &mut Engine, server: &mut FakeServer) -> Result<(), Error> {
    while let Some(message) = engine.next_request() {
        engine.receive(server.handle(&message))?;
    }
    Ok(())
}

pub fn synced(server: &mut FakeServer) -> Engine {
    let mut engine = Engine::new(ACCOUNT);
    sync(&mut engine, server).expect("a first sync");
    engine
}

pub fn login(site: &str, username: &str, password: &str) -> PasswordSpecificsData {
    PasswordSpecificsData {
        signon_realm: Some(format!("https://{site}/")),
        origin: Some(format!("https://{site}/login")),
        username_value: Some(username.to_owned()),
        password_value: Some(password.to_owned()),
        date_created: Some(13_000_000_000_000_000),
        ..PasswordSpecificsData::default()
    }
}

pub fn sealed(key: &Nigori, plain: &[u8]) -> EncryptedData {
    EncryptedData {
        key_name: Some(key.name()),
        blob: Some(key.encrypt(plain)),
    }
}

pub fn keybag(wrapping: &Nigori, keys: &[&Nigori]) -> EncryptedData {
    let keys = EncryptionKeys {
        key: keys
            .iter()
            .map(|key| NigoriKey {
                encryption_key: Some(key.encryption_key().to_vec()),
                mac_key: Some(key.mac_key().to_vec()),
            })
            .collect(),
        cross_user_sharing_private_key: Vec::new(),
    };
    sealed(wrapping, &keys.encode_to_vec())
}

pub fn passphrase_key(passphrase: &str, salt: &[u8]) -> Nigori {
    Nigori::derive(
        &Derivation::Scrypt {
            salt: salt.to_vec(),
        },
        passphrase.as_bytes(),
    )
}

fn kind(entity: &SyncEntity) -> i32 {
    match entity
        .specifics
        .as_ref()
        .is_some_and(|specifics| specifics.nigori.is_some())
    {
        true => NIGORI,
        false => PASSWORDS,
    }
}

fn token(token: &[u8]) -> i64 {
    std::str::from_utf8(token)
        .ok()
        .and_then(|token| token.parse().ok())
        .unwrap_or(0)
}
