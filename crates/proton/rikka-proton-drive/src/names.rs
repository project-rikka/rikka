use rikka_proton_names::{numbered, split_extension};
use serde::{Deserialize, Serialize};

use crate::client::DriveClient;
use crate::error::{Error, Result};
use crate::folder::Folder;
use crate::identifier::{LinkId, RevisionId, VolumeId};

const NAMES_PER_REQUEST: u32 = 10;
const MAX_NAME_NUMBER: u32 = 1000;

#[derive(Debug, Clone)]
pub struct HashAvailability {
    available: Vec<String>,
    pending: Vec<PendingHash>,
}

impl HashAvailability {
    #[must_use]
    pub fn available(&self) -> &[String] {
        &self.available
    }

    #[must_use]
    pub fn pending(&self) -> &[PendingHash] {
        &self.pending
    }

    #[must_use]
    pub fn is_available(&self, hash: &str) -> bool {
        self.available.iter().any(|available| available == hash)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PendingHash {
    #[serde(rename = "Hash")]
    hash: String,
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(rename = "RevisionID")]
    revision_id: RevisionId,
    #[serde(rename = "ClientUID", default)]
    client_uid: Option<String>,
}

impl PendingHash {
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    #[must_use]
    pub fn link_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn revision_id(&self) -> &RevisionId {
        &self.revision_id
    }

    #[must_use]
    pub fn client_uid(&self) -> Option<&str> {
        self.client_uid.as_deref()
    }
}

impl DriveClient {
    pub async fn available_hashes(
        &self,
        volume: &VolumeId,
        folder: &LinkId,
        hashes: &[String],
    ) -> Result<HashAvailability> {
        let request = AvailableRequest {
            hashes,
            client_uids: self.client_uid().map(|uid| vec![uid]),
        };
        let response: AvailableResponse = self
            .api()
            .post(format!(
                "drive/v2/volumes/{volume}/links/{folder}/checkAvailableHashes"
            ))
            .json(&request)
            .send()
            .await?;

        Ok(HashAvailability {
            available: response.available,
            pending: response.pending,
        })
    }

    pub async fn available_name(
        &self,
        volume: &VolumeId,
        folder: &Folder,
        name: &str,
    ) -> Result<String> {
        let (stem, extension) = split_extension(name);
        self.find_available_name(volume, folder, name, stem, extension)
            .await
    }

    pub async fn available_folder_name(
        &self,
        volume: &VolumeId,
        folder: &Folder,
        name: &str,
    ) -> Result<String> {
        self.find_available_name(volume, folder, name, name, "")
            .await
    }

    async fn find_available_name(
        &self,
        volume: &VolumeId,
        folder: &Folder,
        name: &str,
        stem: &str,
        extension: &str,
    ) -> Result<String> {
        let hash_key = folder.hash_key()?;

        let mut first = 1;
        while first < MAX_NAME_NUMBER {
            let mut candidates: Vec<String> = match first {
                1 => vec![name.to_owned()],
                _ => Vec::new(),
            };
            candidates.extend(
                (first..first + NAMES_PER_REQUEST).map(|number| numbered(stem, number, extension)),
            );
            let hashes: Vec<String> = candidates
                .iter()
                .map(|candidate| hash_key.hash(candidate))
                .collect();

            let availability = self.available_hashes(volume, folder.id(), &hashes).await?;
            if let Some(index) = hashes
                .iter()
                .position(|hash| availability.is_available(hash))
            {
                return Ok(candidates.swap_remove(index));
            }
            first += NAMES_PER_REQUEST;
        }

        Err(Error::NoAvailableName(name.to_owned()))
    }
}

#[derive(Debug, Serialize)]
struct AvailableRequest<'a> {
    #[serde(rename = "Hashes")]
    hashes: &'a [String],
    #[serde(rename = "ClientUID")]
    client_uids: Option<Vec<&'a str>>,
}

#[derive(Debug, Deserialize)]
struct AvailableResponse {
    #[serde(rename = "AvailableHashes", default)]
    available: Vec<String>,
    #[serde(rename = "PendingHashes", default)]
    pending: Vec<PendingHash>,
}
