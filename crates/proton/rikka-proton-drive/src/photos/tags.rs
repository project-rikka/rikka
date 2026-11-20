use serde::Serialize;

use crate::client::DriveClient;
use crate::error::Result;
use crate::identifier::{LinkId, VolumeId};
use crate::photo_properties::PhotoTag;

impl DriveClient {
    pub async fn tag_photo(
        &self,
        volume: &VolumeId,
        photo: &LinkId,
        tags: &[PhotoTag],
    ) -> Result<()> {
        self.api()
            .post(tags_path(volume, photo))
            .json(&TagsRequest { tags })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn untag_photo(
        &self,
        volume: &VolumeId,
        photo: &LinkId,
        tags: &[PhotoTag],
    ) -> Result<()> {
        self.api()
            .delete(tags_path(volume, photo))
            .json(&TagsRequest { tags })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn unfavorite_photo(&self, volume: &VolumeId, photo: &LinkId) -> Result<()> {
        self.untag_photo(volume, photo, &[PhotoTag::Favorites])
            .await
    }
}

fn tags_path(volume: &VolumeId, photo: &LinkId) -> String {
    format!("drive/photos/volumes/{volume}/links/{photo}/tags")
}

#[derive(Debug, Serialize)]
struct TagsRequest<'a> {
    #[serde(rename = "Tags")]
    tags: &'a [PhotoTag],
}
