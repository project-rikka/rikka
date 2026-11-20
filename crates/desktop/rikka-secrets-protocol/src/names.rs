pub const BUS_NAME: &str = "org.freedesktop.secrets";

pub const SERVICE_PATH: &str = "/org/freedesktop/secrets";
pub const COLLECTION_PREFIX: &str = "/org/freedesktop/secrets/collection/";
pub const ALIAS_PREFIX: &str = "/org/freedesktop/secrets/aliases/";
pub const SESSION_PREFIX: &str = "/org/freedesktop/secrets/session/";
pub const PROMPT_PREFIX: &str = "/org/freedesktop/secrets/prompt/";
pub const NO_OBJECT: &str = "/";

pub const SERVICE_INTERFACE: &str = "org.freedesktop.Secret.Service";
pub const COLLECTION_INTERFACE: &str = "org.freedesktop.Secret.Collection";
pub const ITEM_INTERFACE: &str = "org.freedesktop.Secret.Item";
pub const PROMPT_INTERFACE: &str = "org.freedesktop.Secret.Prompt";
pub const KEYRING_INTERFACE: &str = "os.rikka.Keyring1";

pub const ITEM_LABEL: &str = "org.freedesktop.Secret.Item.Label";
pub const ITEM_ATTRIBUTES: &str = "org.freedesktop.Secret.Item.Attributes";
pub const COLLECTION_LABEL: &str = "org.freedesktop.Secret.Collection.Label";

pub const DEFAULT_ALIAS: &str = "default";
pub const SESSION_ALIAS: &str = "session";
pub const LOGIN_COLLECTION: &str = "login";
pub const SESSION_COLLECTION: &str = "session";

pub const SCHEMA_ATTRIBUTE: &str = "xdg:schema";
pub const BINARY_CONTENT: &str = "application/octet-stream";
pub const TEXT_CONTENT: &str = "text/plain";

pub const IS_LOCKED: &str = "org.freedesktop.Secret.Error.IsLocked";
pub const NO_SESSION: &str = "org.freedesktop.Secret.Error.NoSession";
pub const NO_SUCH_OBJECT: &str = "org.freedesktop.Secret.Error.NoSuchObject";
