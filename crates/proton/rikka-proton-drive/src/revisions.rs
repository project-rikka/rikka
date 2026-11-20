use crate::client::DriveClient;
use crate::error::Result;
use crate::identifier::{LinkId, RevisionId, VolumeId};
use crate::revision::{RevisionInfo, RevisionState};

impl DriveClient {
    pub async fn revisions(&self, volume: &VolumeId, file: &LinkId) -> Result<Vec<RevisionInfo>> {
        let revisions: Vec<RevisionInfo> = self
            .api()
            .get(format!("drive/v2/volumes/{volume}/files/{file}/revisions"))
            .send_field_or_default("Revisions")
            .await?;

        Ok(revisions
            .into_iter()
            .filter(|revision| {
                matches!(
                    revision.state(),
                    RevisionState::Active | RevisionState::Obsolete
                )
            })
            .collect())
    }

    pub async fn restore_revision(
        &self,
        volume: &VolumeId,
        file: &LinkId,
        revision: &RevisionId,
    ) -> Result<()> {
        self.api()
            .post(format!(
                "drive/v2/volumes/{volume}/files/{file}/revisions/{revision}/restore"
            ))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_revision(
        &self,
        volume: &VolumeId,
        file: &LinkId,
        revision: &RevisionId,
    ) -> Result<()> {
        self.api()
            .delete(format!(
                "drive/v2/volumes/{volume}/files/{file}/revisions/{revision}"
            ))
            .send_unit()
            .await?;

        Ok(())
    }
}
