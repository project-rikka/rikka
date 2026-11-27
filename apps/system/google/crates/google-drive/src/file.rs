use serde::{Deserialize, Serialize};

pub(crate) const FOLDER: &str = "application/vnd.google-apps.folder";
const SHORTCUT: &str = "application/vnd.google-apps.shortcut";
const NATIVE: &str = "application/vnd.google-apps.";

pub(crate) const FIELDS: &str = "id,name,mimeType,size,modifiedTime,createdTime,parents,starred,\
    trashed,shared,ownedByMe,version,thumbnailLink,hasThumbnail,webViewLink,driveId,\
    owners(displayName),shortcutDetails(targetId,targetMimeType),\
    capabilities(canEdit,canRename,canTrash,canDelete,canAddChildren,canDownload,canMoveItemWithinDrive,canCopy)";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Folder,
    Document,
    Spreadsheet,
    Presentation,
    Drawing,
    Form,
    Site,
    Image,
    Video,
    Audio,
    Pdf,
    Archive,
    Text,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Capabilities {
    pub can_edit: bool,
    pub can_rename: bool,
    pub can_trash: bool,
    pub can_delete: bool,
    pub can_add_children: bool,
    pub can_download: bool,
    pub can_move_item_within_drive: bool,
    pub can_copy: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcut {
    #[serde(default)]
    pub target_id: String,
    #[serde(default)]
    pub target_mime_type: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Owner {
    #[serde(default)]
    pub display_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default, with = "decimal")]
    pub size: Option<u64>,
    #[serde(default, with = "moment")]
    pub modified_time: Option<i64>,
    #[serde(default, with = "moment")]
    pub created_time: Option<i64>,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub trashed: bool,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub owned_by_me: bool,
    #[serde(default, with = "decimal")]
    pub version: Option<u64>,
    #[serde(default)]
    pub thumbnail_link: Option<String>,
    #[serde(default)]
    pub has_thumbnail: bool,
    #[serde(default)]
    pub web_view_link: Option<String>,
    #[serde(default)]
    pub drive_id: Option<String>,
    #[serde(default)]
    pub owners: Vec<Owner>,
    #[serde(default)]
    pub shortcut_details: Option<Shortcut>,
    #[serde(default)]
    pub capabilities: Capabilities,
}

impl File {
    #[must_use]
    pub fn target_id(&self) -> &str {
        match &self.shortcut_details {
            Some(shortcut) if !shortcut.target_id.is_empty() => &shortcut.target_id,
            _ => &self.id,
        }
    }

    #[must_use]
    pub fn target_mime_type(&self) -> &str {
        match &self.shortcut_details {
            Some(shortcut) if self.mime_type == SHORTCUT => &shortcut.target_mime_type,
            _ => &self.mime_type,
        }
    }

    #[must_use]
    pub fn is_shortcut(&self) -> bool {
        self.mime_type == SHORTCUT
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.target_mime_type() == FOLDER
    }

    #[must_use]
    pub fn is_native(&self) -> bool {
        let mime = self.target_mime_type();
        mime.starts_with(NATIVE) && mime != FOLDER
    }

    #[must_use]
    pub fn kind(&self) -> Kind {
        let mime = self.target_mime_type();
        if let Some(native) = mime.strip_prefix(NATIVE) {
            return match native {
                "folder" => Kind::Folder,
                "document" => Kind::Document,
                "spreadsheet" => Kind::Spreadsheet,
                "presentation" => Kind::Presentation,
                "drawing" => Kind::Drawing,
                "form" => Kind::Form,
                "site" => Kind::Site,
                _ => Kind::Other,
            };
        }
        match mime.split_once('/') {
            Some(("image", _)) => Kind::Image,
            Some(("video", _)) => Kind::Video,
            Some(("audio", _)) => Kind::Audio,
            Some(("text", _)) => Kind::Text,
            Some(("application", "pdf")) => Kind::Pdf,
            Some((
                "application",
                "zip" | "x-tar" | "gzip" | "x-7z-compressed" | "x-rar-compressed" | "vnd.rar"
                | "x-bzip2" | "x-xz" | "zstd",
            )) => Kind::Archive,
            Some(("application", "json" | "xml" | "x-yaml" | "javascript" | "x-sh")) => Kind::Text,
            _ => Kind::Other,
        }
    }

    #[must_use]
    pub fn thumbnail(&self, pixels: u32) -> Option<String> {
        let link = self
            .thumbnail_link
            .as_deref()
            .filter(|_| self.has_thumbnail)?;
        let base = link.rsplit_once("=s").map_or(link, |(base, _)| base);
        Some(format!("{base}=s{pixels}"))
    }

    #[must_use]
    pub fn owner(&self) -> &str {
        self.owners
            .first()
            .map_or("", |owner| owner.display_name.as_str())
    }
}

mod decimal {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        value: &Option<u64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.collect_str(value),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        Ok(text.and_then(|text| text.parse().ok()))
    }
}

mod moment {
    use jiff::Timestamp;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        value: &Option<i64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value.and_then(|millis| Timestamp::from_millisecond(millis).ok()) {
            Some(moment) => serializer.collect_str(&moment),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<i64>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        Ok(text
            .and_then(|text| text.parse::<Timestamp>().ok())
            .map(Timestamp::as_millisecond))
    }
}
