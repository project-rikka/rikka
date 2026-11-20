use serde::{Deserialize, Serialize};

use crate::client::DriveClient;
use crate::error::Result;
use crate::identifier::{LinkId, RevisionId, VolumeId};
use crate::link::LinkState;
use crate::photos::library::PhotoLibrary;

#[derive(Debug, Clone, Deserialize)]
pub struct PhotoDuplicate {
    #[serde(rename = "Hash")]
    name_hash: String,
    #[serde(rename = "ContentHash", default)]
    content_hash: Option<String>,
    #[serde(rename = "LinkState", default)]
    state: LinkState,
    #[serde(rename = "LinkID", default)]
    link_id: Option<LinkId>,
    #[serde(rename = "RevisionID", default)]
    revision_id: Option<RevisionId>,
    #[serde(rename = "ClientUID", default)]
    client_uid: Option<String>,
}

impl PhotoDuplicate {
    #[must_use]
    pub fn name_hash(&self) -> &str {
        &self.name_hash
    }

    #[must_use]
    pub fn content_hash(&self) -> Option<&str> {
        self.content_hash.as_deref()
    }

    #[must_use]
    pub fn state(&self) -> LinkState {
        self.state
    }

    #[must_use]
    pub fn link_id(&self) -> Option<&LinkId> {
        self.link_id.as_ref()
    }

    #[must_use]
    pub fn revision_id(&self) -> Option<&RevisionId> {
        self.revision_id.as_ref()
    }

    #[must_use]
    pub fn client_uid(&self) -> Option<&str> {
        self.client_uid.as_deref()
    }
}

impl DriveClient {
    pub async fn photo_duplicates(
        &self,
        volume: &VolumeId,
        name_hashes: &[String],
    ) -> Result<Vec<PhotoDuplicate>> {
        Ok(self
            .api()
            .post(format!("drive/volumes/{volume}/photos/duplicates"))
            .json(&DuplicatesRequest { name_hashes })
            .send_field_or_default("DuplicateHashes")
            .await?)
    }

    pub async fn find_photo_duplicates(
        &self,
        library: &PhotoLibrary,
        name: &str,
        sha1: &str,
    ) -> Result<Vec<LinkId>> {
        let hash_key = library.root().hash_key()?;
        let name_hash = hash_key.hash(name);
        let content_hash = hash_key.hash(&sha1.to_ascii_lowercase());

        let found = self
            .photo_duplicates(library.volume_id(), std::slice::from_ref(&name_hash))
            .await?;

        Ok(found
            .into_iter()
            .filter(|duplicate| {
                duplicate.state == LinkState::Active
                    && duplicate.name_hash == name_hash
                    && duplicate.content_hash.as_deref() == Some(content_hash.as_str())
            })
            .filter_map(|duplicate| duplicate.link_id)
            .collect())
    }
}

#[derive(Debug, Serialize)]
struct DuplicatesRequest<'a> {
    #[serde(rename = "NameHashes")]
    name_hashes: &'a [String],
}
