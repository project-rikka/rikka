use serde::Deserialize;

use crate::identifier::{ShareId, ShareUrlId};

#[derive(Debug, Clone, Deserialize)]
pub struct SharingDetails {
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "ShareURLID", default)]
    share_url_id: Option<ShareUrlId>,
}

impl SharingDetails {
    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn share_url_id(&self) -> Option<&ShareUrlId> {
        self.share_url_id.as_ref()
    }

    #[must_use]
    pub fn is_shared_by_url(&self) -> bool {
        self.share_url_id.is_some()
    }
}
