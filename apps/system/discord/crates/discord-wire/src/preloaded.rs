use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use discord_core::{Folder, SpaceId, Status};
use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct Preloaded {
    #[prost(message, optional, tag = "11")]
    pub status: Option<StatusSettings>,
    #[prost(message, optional, tag = "12")]
    pub localization: Option<Localization>,
    #[prost(message, optional, tag = "14")]
    pub guild_folders: Option<GuildFolders>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StatusSettings {
    #[prost(message, optional, tag = "1")]
    pub status: Option<StringValue>,
    #[prost(message, optional, tag = "2")]
    pub custom_status: Option<CustomStatus>,
}

#[derive(Clone, PartialEq, Message)]
pub struct CustomStatus {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(fixed64, tag = "2")]
    pub emoji_id: u64,
    #[prost(string, tag = "3")]
    pub emoji_name: String,
    #[prost(fixed64, tag = "4")]
    pub expires_at_ms: u64,
}

#[derive(Clone, PartialEq, Message)]
pub struct Localization {
    #[prost(message, optional, tag = "1")]
    pub locale: Option<StringValue>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GuildFolders {
    #[prost(message, repeated, tag = "1")]
    pub folders: Vec<GuildFolder>,
}

#[derive(Clone, PartialEq, Message)]
pub struct GuildFolder {
    #[prost(fixed64, repeated, tag = "1")]
    pub guild_ids: Vec<u64>,
    #[prost(message, optional, tag = "2")]
    pub id: Option<Int64Value>,
    #[prost(message, optional, tag = "3")]
    pub name: Option<StringValue>,
    #[prost(message, optional, tag = "4")]
    pub color: Option<UInt64Value>,
}

#[derive(Clone, PartialEq, Message)]
pub struct StringValue {
    #[prost(string, tag = "1")]
    pub value: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Int64Value {
    #[prost(int64, tag = "1")]
    pub value: i64,
}

#[derive(Clone, PartialEq, Message)]
pub struct UInt64Value {
    #[prost(uint64, tag = "1")]
    pub value: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Coverage {
    #[default]
    Complete,
    Partial,
}

impl Preloaded {
    pub fn decode_base64(encoded: &str) -> Preloaded {
        STANDARD
            .decode(encoded.trim())
            .ok()
            .and_then(|bytes| Preloaded::decode(bytes.as_slice()).ok())
            .unwrap_or_default()
    }

    pub fn absorb(&mut self, update: Preloaded, coverage: Coverage) {
        if coverage == Coverage::Complete {
            let folders = self.guild_folders.take();
            *self = update;
            if self
                .guild_folders
                .as_ref()
                .is_none_or(|settings| settings.folders.is_empty())
            {
                self.guild_folders = folders;
            }
            return;
        }
        if let Some(status) = update.status {
            let mut merged = self.status.take().unwrap_or_default();
            if status.status.is_some() {
                merged.status = status.status;
            }
            if status.custom_status.is_some() {
                merged.custom_status = status.custom_status;
            }
            self.status = Some(merged);
        }
        if update.localization.is_some() {
            self.localization = update.localization;
        }
        if update.guild_folders.is_some() {
            self.guild_folders = update.guild_folders;
        }
    }

    pub fn folders(&self) -> Vec<Folder> {
        self.guild_folders
            .iter()
            .flat_map(|folders| &folders.folders)
            .map(|folder| Folder {
                id: folder.id.as_ref().map(|id| id.value.to_string()),
                name: folder
                    .name
                    .as_ref()
                    .map(|name| name.value.clone())
                    .filter(|name| !name.is_empty()),
                color: folder
                    .color
                    .as_ref()
                    .and_then(|color| u32::try_from(color.value).ok()),
                spaces: folder
                    .guild_ids
                    .iter()
                    .map(|&id| SpaceId::new(id))
                    .collect(),
            })
            .collect()
    }

    pub fn status(&self) -> Option<Status> {
        let status = self.status.as_ref()?.status.as_ref()?;
        Some(status_named(&status.value))
    }

    pub fn custom_status(&self, now_ms: i64) -> Option<&CustomStatus> {
        let custom = self.status.as_ref()?.custom_status.as_ref()?;
        let live = custom.expires_at_ms == 0
            || i64::try_from(custom.expires_at_ms).is_ok_and(|expires| expires > now_ms);
        (live && (!custom.text.is_empty() || !custom.emoji_name.is_empty())).then_some(custom)
    }

    pub fn locale(&self) -> Option<&str> {
        self.localization
            .as_ref()?
            .locale
            .as_ref()
            .map(|locale| locale.value.as_str())
            .filter(|locale| !locale.is_empty())
    }

    pub fn status_patch(status: Status) -> String {
        let patch = Preloaded {
            status: Some(StatusSettings {
                status: Some(StringValue {
                    value: status_name(status).to_owned(),
                }),
                custom_status: None,
            }),
            localization: None,
            guild_folders: None,
        };
        STANDARD.encode(patch.encode_to_vec())
    }
}

pub fn status_named(name: &str) -> Status {
    match name {
        "online" => Status::Online,
        "idle" => Status::Idle,
        "dnd" => Status::Busy,
        "invisible" => Status::Invisible,
        _ => Status::Offline,
    }
}

pub fn status_name(status: Status) -> &'static str {
    match status {
        Status::Online => "online",
        Status::Idle => "idle",
        Status::Busy => "dnd",
        Status::Invisible | Status::Offline => "invisible",
    }
}
