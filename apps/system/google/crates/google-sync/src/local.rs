use prost::Message;

use crate::proto::{EncryptedData, NigoriKey, SyncEntity};

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub(crate) struct LocalNigori {
    #[prost(message, repeated, tag = "1")]
    pub(crate) keys: Vec<NigoriKey>,
    #[prost(string, optional, tag = "2")]
    pub(crate) primary: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub(crate) pending_keybag: Option<EncryptedData>,
    #[prost(message, optional, tag = "4")]
    pub(crate) pending_keystore_token: Option<EncryptedData>,
    #[prost(bytes = "vec", repeated, tag = "5")]
    pub(crate) keystore_keys: Vec<Vec<u8>>,
    #[prost(int32, optional, tag = "6")]
    pub(crate) passphrase_type: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub(crate) scrypt_salt: Option<Vec<u8>>,
    #[prost(bool, tag = "8")]
    pub(crate) pbkdf2: bool,
    #[prost(bool, tag = "9")]
    pub(crate) encrypt_everything: bool,
    #[prost(int64, tag = "10")]
    pub(crate) keystore_migration_time: i64,
    #[prost(int64, tag = "11")]
    pub(crate) custom_passphrase_time: i64,
}

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub(crate) struct LocalEntity {
    #[prost(string, tag = "1")]
    pub(crate) tag: String,
    #[prost(string, tag = "2")]
    pub(crate) server_id: String,
    #[prost(int64, tag = "3")]
    pub(crate) version: i64,
    #[prost(int64, tag = "4")]
    pub(crate) ctime: i64,
    #[prost(int64, tag = "5")]
    pub(crate) mtime: i64,
    #[prost(bytes = "vec", tag = "6")]
    pub(crate) data: Vec<u8>,
    #[prost(bool, tag = "7")]
    pub(crate) deleted: bool,
    #[prost(uint64, tag = "8")]
    pub(crate) sequence: u64,
    #[prost(uint64, tag = "9")]
    pub(crate) acked: u64,
}

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub(crate) struct LocalState {
    #[prost(string, tag = "1")]
    pub(crate) cache_guid: String,
    #[prost(string, optional, tag = "2")]
    pub(crate) birthday: Option<String>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub(crate) bag_of_chips: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub(crate) nigori_marker: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "5")]
    pub(crate) passwords_marker: Option<Vec<u8>>,
    #[prost(message, optional, tag = "6")]
    pub(crate) nigori: Option<LocalNigori>,
    #[prost(message, repeated, tag = "7")]
    pub(crate) entities: Vec<LocalEntity>,
    #[prost(message, repeated, tag = "8")]
    pub(crate) stashed: Vec<SyncEntity>,
    #[prost(string, tag = "9")]
    pub(crate) nigori_id: String,
    #[prost(int64, tag = "10")]
    pub(crate) nigori_version: i64,
    #[prost(bool, tag = "11")]
    pub(crate) nigori_unsynced: bool,
    #[prost(uint32, tag = "12")]
    pub(crate) poll_interval: u32,
    #[prost(uint32, tag = "13")]
    pub(crate) commit_batch: u32,
}
