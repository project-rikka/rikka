use jiff::Timestamp;
use rikka_proton_drive::{File, LinkId, LinkState, Node, ThumbnailType};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: LinkId,
    pub name: String,
    pub folder: bool,
    pub size: i64,
    pub media_type: String,
    #[serde(with = "rikka_proton_api::unix_seconds")]
    pub modified: Timestamp,
    pub trashed: bool,
    #[serde(default)]
    pub pictured: bool,
}

impl Entry {
    #[must_use]
    pub fn from_node(node: &Node) -> Entry {
        let link = node.link();
        let file = node.as_file();
        Entry {
            id: node.id().clone(),
            name: node.name().to_owned(),
            folder: node.is_folder(),
            size: link.size(),
            media_type: file
                .map(|file| file.media_type().to_owned())
                .unwrap_or_default(),
            modified: node
                .modified_at()
                .or_else(|| link.modified_at())
                .unwrap_or_default(),
            trashed: link.state() == LinkState::Trashed,
            pictured: file
                .and_then(File::active_revision)
                .is_some_and(|revision| revision.has_thumbnail(ThumbnailType::Preview)),
        }
    }

    #[must_use]
    pub fn suffix(&self) -> Option<&str> {
        let at = self.name.rfind('.')?;
        let suffix = &self.name[at + 1..];
        (!suffix.is_empty() && at > 0).then_some(suffix)
    }

    #[must_use]
    pub fn kind_key(&self) -> &str {
        match self.folder {
            true => "",
            false => self.suffix().unwrap_or(&self.media_type),
        }
    }
}
