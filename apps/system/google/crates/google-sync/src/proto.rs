use prost::Message;

pub const PASSWORDS: i32 = 45873;
pub const NIGORI: i32 = 47745;

pub(crate) const PROTOCOL_VERSION: i32 = 99;
pub(crate) const NIGORI_TAG: &str = "google_chrome_nigori";
pub(crate) const NIGORI_NAME: &str = "Nigori";
pub(crate) const ENCRYPTED_NAME: &str = "encrypted";

pub(crate) mod contents {
    pub(crate) const COMMIT: i32 = 1;
    pub(crate) const GET_UPDATES: i32 = 2;
}

pub(crate) mod origin {
    pub(crate) const PERIODIC: i32 = 4;
    pub(crate) const NEW_CLIENT: i32 = 9;
    pub(crate) const GU_TRIGGER: i32 = 12;
}

pub(crate) mod error_type {
    pub(crate) const SUCCESS: i32 = 0;
    pub(crate) const NOT_MY_BIRTHDAY: i32 = 2;
    pub(crate) const THROTTLED: i32 = 3;
    pub(crate) const TRANSIENT_ERROR: i32 = 8;
    pub(crate) const MIGRATION_DONE: i32 = 9;
    pub(crate) const DISABLED_BY_ADMIN: i32 = 10;
    pub(crate) const PARTIAL_FAILURE: i32 = 12;
    pub(crate) const CLIENT_DATA_OBSOLETE: i32 = 13;
    pub(crate) const ENCRYPTION_OBSOLETE: i32 = 14;
}

pub(crate) mod commit_result {
    pub(crate) const SUCCESS: i32 = 1;
    pub(crate) const CONFLICT: i32 = 2;
    pub(crate) const INVALID_MESSAGE: i32 = 4;
}

pub mod passphrase {
    pub const IMPLICIT: i32 = 1;
    pub const KEYSTORE: i32 = 2;
    pub const FROZEN_IMPLICIT: i32 = 3;
    pub const CUSTOM: i32 = 4;
    pub const TRUSTED_VAULT: i32 = 5;
}

pub(crate) mod derivation {
    pub(crate) const UNSPECIFIED: i32 = 0;
    pub(crate) const PBKDF2_HMAC_SHA1_1003: i32 = 1;
    pub(crate) const SCRYPT_8192_8_11: i32 = 2;
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct EncryptedData {
    #[prost(string, optional, tag = "1")]
    pub key_name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub blob: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub struct NigoriKey {
    #[prost(bytes = "vec", optional, tag = "3")]
    pub encryption_key: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub mac_key: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub struct EncryptionKeys {
    #[prost(message, repeated, tag = "2")]
    pub key: Vec<NigoriKey>,
    #[prost(bytes = "vec", repeated, tag = "3")]
    pub cross_user_sharing_private_key: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct NigoriSpecifics {
    #[prost(message, optional, tag = "1")]
    pub encryption_keybag: Option<EncryptedData>,
    #[prost(bool, optional, tag = "2")]
    pub keybag_is_frozen: Option<bool>,
    #[prost(bool, optional, tag = "24")]
    pub encrypt_everything: Option<bool>,
    #[prost(int32, optional, tag = "30")]
    pub passphrase_type: Option<i32>,
    #[prost(message, optional, tag = "31")]
    pub keystore_decryptor_token: Option<EncryptedData>,
    #[prost(int64, optional, tag = "32")]
    pub keystore_migration_time: Option<i64>,
    #[prost(int64, optional, tag = "33")]
    pub custom_passphrase_time: Option<i64>,
    #[prost(int32, optional, tag = "45")]
    pub custom_passphrase_key_derivation_method: Option<i32>,
    #[prost(string, optional, tag = "46")]
    pub custom_passphrase_key_derivation_salt: Option<String>,
    #[prost(bytes = "vec", optional, tag = "50")]
    pub trusted_vault_debug_info: Option<Vec<u8>>,
    #[prost(bytes = "vec", optional, tag = "53")]
    pub cross_user_sharing_public_key: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub struct Note {
    #[prost(string, optional, tag = "1")]
    pub unique_display_name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub value: Option<String>,
    #[prost(int64, optional, tag = "3")]
    pub date_created_windows_epoch_micros: Option<i64>,
    #[prost(bool, optional, tag = "4")]
    pub hide_by_default: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub struct Notes {
    #[prost(message, repeated, tag = "1")]
    pub note: Vec<Note>,
}

#[derive(Clone, PartialEq, Eq, Message)]
#[prost(skip_debug)]
pub struct PasswordSpecificsData {
    #[prost(int32, optional, tag = "1")]
    pub scheme: Option<i32>,
    #[prost(string, optional, tag = "2")]
    pub signon_realm: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub origin: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub action: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub username_element: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub username_value: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub password_element: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub password_value: Option<String>,
    #[prost(bool, optional, tag = "9")]
    pub ssl_valid: Option<bool>,
    #[prost(bool, optional, tag = "10")]
    pub preferred: Option<bool>,
    #[prost(int64, optional, tag = "11")]
    pub date_created: Option<i64>,
    #[prost(bool, optional, tag = "12")]
    pub blacklisted: Option<bool>,
    #[prost(int32, optional, tag = "13")]
    pub r#type: Option<i32>,
    #[prost(int32, optional, tag = "14")]
    pub times_used: Option<i32>,
    #[prost(string, optional, tag = "15")]
    pub display_name: Option<String>,
    #[prost(string, optional, tag = "16")]
    pub avatar_url: Option<String>,
    #[prost(string, optional, tag = "17")]
    pub federation_url: Option<String>,
    #[prost(int64, optional, tag = "18")]
    pub date_last_used: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "19")]
    pub password_issues: Option<Vec<u8>>,
    #[prost(int64, optional, tag = "20")]
    pub date_password_modified_windows_epoch_micros: Option<i64>,
    #[prost(message, optional, tag = "22")]
    pub notes: Option<Notes>,
    #[prost(string, optional, tag = "23")]
    pub sender_email: Option<String>,
    #[prost(string, optional, tag = "24")]
    pub sender_name: Option<String>,
    #[prost(int64, optional, tag = "25")]
    pub date_received_windows_epoch_micros: Option<i64>,
    #[prost(bool, optional, tag = "26")]
    pub sharing_notification_displayed: Option<bool>,
    #[prost(string, optional, tag = "27")]
    pub sender_profile_image_url: Option<String>,
    #[prost(int64, optional, tag = "28")]
    pub date_last_filled_windows_epoch_micros: Option<i64>,
    #[prost(bool, optional, tag = "29")]
    pub actor_login_approved: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct PasswordSpecificsMetadata {
    #[prost(string, optional, tag = "1")]
    pub url: Option<String>,
    #[prost(bool, optional, tag = "2")]
    pub blacklisted: Option<bool>,
    #[prost(int64, optional, tag = "3")]
    pub date_last_used_windows_epoch_micros: Option<i64>,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub password_issues: Option<Vec<u8>>,
    #[prost(int32, optional, tag = "13")]
    pub r#type: Option<i32>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct PasswordSpecifics {
    #[prost(message, optional, tag = "1")]
    pub encrypted: Option<EncryptedData>,
    #[prost(message, optional, tag = "3")]
    pub unencrypted_metadata: Option<PasswordSpecificsMetadata>,
    #[prost(message, optional, tag = "5")]
    pub encrypted_notes_backup: Option<EncryptedData>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct EntitySpecifics {
    #[prost(message, optional, tag = "1")]
    pub encrypted: Option<EncryptedData>,
    #[prost(message, optional, tag = "45873")]
    pub password: Option<PasswordSpecifics>,
    #[prost(message, optional, tag = "47745")]
    pub nigori: Option<NigoriSpecifics>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct SyncEntity {
    #[prost(string, optional, tag = "1")]
    pub id_string: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub parent_id_string: Option<String>,
    #[prost(int64, optional, tag = "4")]
    pub version: Option<i64>,
    #[prost(int64, optional, tag = "5")]
    pub mtime: Option<i64>,
    #[prost(int64, optional, tag = "6")]
    pub ctime: Option<i64>,
    #[prost(string, optional, tag = "7")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub non_unique_name: Option<String>,
    #[prost(string, optional, tag = "10")]
    pub server_defined_unique_tag: Option<String>,
    #[prost(bool, optional, tag = "18")]
    pub deleted: Option<bool>,
    #[prost(string, optional, tag = "19")]
    pub originator_cache_guid: Option<String>,
    #[prost(string, optional, tag = "20")]
    pub originator_client_item_id: Option<String>,
    #[prost(message, optional, tag = "21")]
    pub specifics: Option<EntitySpecifics>,
    #[prost(bool, optional, tag = "22")]
    pub folder: Option<bool>,
    #[prost(string, optional, tag = "23")]
    pub client_tag_hash: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GetUpdateTriggers {
    #[prost(string, repeated, tag = "1")]
    pub notification_hint: Vec<String>,
    #[prost(bool, optional, tag = "2")]
    pub client_dropped_hints: Option<bool>,
    #[prost(bool, optional, tag = "3")]
    pub invalidations_out_of_sync: Option<bool>,
    #[prost(int64, optional, tag = "4")]
    pub local_modification_nudges: Option<i64>,
    #[prost(int64, optional, tag = "5")]
    pub datatype_refresh_nudges: Option<i64>,
    #[prost(bool, optional, tag = "6")]
    pub server_dropped_hints: Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub initial_sync_in_progress: Option<bool>,
    #[prost(bool, optional, tag = "8")]
    pub sync_for_resolve_conflict_in_progress: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GarbageCollectionDirective {
    #[prost(int64, optional, tag = "2")]
    pub version_watermark: Option<i64>,
    #[prost(bool, optional, tag = "6")]
    pub clear_metadata: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct DataTypeProgressMarker {
    #[prost(int32, optional, tag = "1")]
    pub data_type_id: Option<i32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub token: Option<Vec<u8>>,
    #[prost(message, optional, tag = "5")]
    pub get_update_triggers: Option<GetUpdateTriggers>,
    #[prost(message, optional, tag = "6")]
    pub gc_directive: Option<GarbageCollectionDirective>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GetUpdatesCallerInfo {
    #[prost(bool, optional, tag = "2")]
    pub notifications_enabled: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GetUpdatesMessage {
    #[prost(message, optional, tag = "2")]
    pub caller_info: Option<GetUpdatesCallerInfo>,
    #[prost(bool, optional, tag = "3")]
    pub fetch_folders: Option<bool>,
    #[prost(message, repeated, tag = "6")]
    pub from_progress_marker: Vec<DataTypeProgressMarker>,
    #[prost(bool, optional, tag = "7")]
    pub streaming: Option<bool>,
    #[prost(bool, optional, tag = "8")]
    pub need_encryption_key: Option<bool>,
    #[prost(int32, optional, tag = "9")]
    pub get_updates_origin: Option<i32>,
    #[prost(bool, optional, tag = "10")]
    pub is_retry: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct ClientConfigParams {
    #[prost(int32, repeated, packed = "false", tag = "1")]
    pub enabled_type_ids: Vec<i32>,
    #[prost(bool, optional, tag = "3")]
    pub cookie_jar_mismatch: Option<bool>,
    #[prost(bool, optional, tag = "4")]
    pub single_client: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct CommitMessage {
    #[prost(message, repeated, tag = "1")]
    pub entries: Vec<SyncEntity>,
    #[prost(string, optional, tag = "2")]
    pub cache_guid: Option<String>,
    #[prost(message, optional, tag = "4")]
    pub config_params: Option<ClientConfigParams>,
    #[prost(string, optional, tag = "6")]
    pub padding: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct ChipBag {
    #[prost(bytes = "vec", optional, tag = "1")]
    pub server_chips: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct ClientStatus {
    #[prost(bool, optional, tag = "2")]
    pub is_sync_feature_enabled: Option<bool>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct ClientToServerMessage {
    #[prost(string, required, tag = "1")]
    pub share: String,
    #[prost(int32, optional, tag = "2")]
    pub protocol_version: Option<i32>,
    #[prost(int32, required, tag = "3")]
    pub message_contents: i32,
    #[prost(message, optional, tag = "4")]
    pub commit: Option<CommitMessage>,
    #[prost(message, optional, tag = "5")]
    pub get_updates: Option<GetUpdatesMessage>,
    #[prost(string, optional, tag = "7")]
    pub store_birthday: Option<String>,
    #[prost(message, optional, tag = "11")]
    pub bag_of_chips: Option<ChipBag>,
    #[prost(message, optional, tag = "13")]
    pub client_status: Option<ClientStatus>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct EntryResponse {
    #[prost(int32, optional, tag = "2")]
    pub response_type: Option<i32>,
    #[prost(string, optional, tag = "3")]
    pub id_string: Option<String>,
    #[prost(int64, optional, tag = "6")]
    pub version: Option<i64>,
    #[prost(string, optional, tag = "9")]
    pub error_message: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct CommitResponse {
    #[prost(group, repeated, tag = "1")]
    pub entryresponse: Vec<EntryResponse>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GetUpdatesResponse {
    #[prost(message, repeated, tag = "1")]
    pub entries: Vec<SyncEntity>,
    #[prost(int64, optional, tag = "4")]
    pub changes_remaining: Option<i64>,
    #[prost(message, repeated, tag = "5")]
    pub new_progress_marker: Vec<DataTypeProgressMarker>,
    #[prost(bytes = "vec", repeated, tag = "6")]
    pub encryption_keys: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct ClientCommand {
    #[prost(int32, optional, tag = "1")]
    pub set_sync_poll_interval: Option<i32>,
    #[prost(int32, optional, tag = "3")]
    pub max_commit_batch_size: Option<i32>,
    #[prost(int32, optional, tag = "5")]
    pub throttle_delay_seconds: Option<i32>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct ResponseError {
    #[prost(int32, optional, tag = "1")]
    pub error_type: Option<i32>,
    #[prost(string, optional, tag = "2")]
    pub error_description: Option<String>,
    #[prost(int32, optional, tag = "4")]
    pub action: Option<i32>,
    #[prost(int32, repeated, packed = "false", tag = "5")]
    pub error_data_type_ids: Vec<i32>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct ClientToServerResponse {
    #[prost(message, optional, tag = "1")]
    pub commit: Option<CommitResponse>,
    #[prost(message, optional, tag = "2")]
    pub get_updates: Option<GetUpdatesResponse>,
    #[prost(int32, optional, tag = "4")]
    pub error_code: Option<i32>,
    #[prost(string, optional, tag = "5")]
    pub error_message: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub store_birthday: Option<String>,
    #[prost(message, optional, tag = "7")]
    pub client_command: Option<ClientCommand>,
    #[prost(message, optional, tag = "13")]
    pub error: Option<ResponseError>,
    #[prost(message, optional, tag = "14")]
    pub new_bag_of_chips: Option<ChipBag>,
}

macro_rules! redacted {
    ($($message:ident),+) => {
        $(impl std::fmt::Debug for $message {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct(stringify!($message))
                    .field("bytes", &self.encoded_len())
                    .finish_non_exhaustive()
            }
        })+
    };
}

redacted!(
    NigoriKey,
    EncryptionKeys,
    Note,
    Notes,
    PasswordSpecificsData
);
