use rikka_proton_api::offload;
use serde::Serialize;

use crate::api_errors;
use crate::bootstrap::{RootBootstrap, ShareBootstrap};
use crate::client::DriveClient;
use crate::error::Result;
use crate::folder::Folder;
use crate::identifier::{ShareId, VolumeId};
use crate::share_keys::ShareKeys;
use crate::upload::{PhotoDetails, Upload};
use crate::volume::Volume;

const ROOT_NAME: &str = "root";

#[derive(Debug, Clone)]
pub struct PhotoLibrary {
    share: ShareKeys,
    root: Folder,
}

impl PhotoLibrary {
    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        self.share.volume_id()
    }

    #[must_use]
    pub fn share(&self) -> &ShareKeys {
        &self.share
    }

    #[must_use]
    pub fn root(&self) -> &Folder {
        &self.root
    }
}

impl DriveClient {
    pub async fn photo_library(&self) -> Result<PhotoLibrary> {
        let response = self
            .api()
            .get("drive/v2/shares/photos")
            .send_field_at::<ShareId>(&["Share", "ShareID"])
            .await;
        let share = match response {
            Ok(share) => share,
            Err(error) if api_errors::is_missing(&error) => {
                self.create_photo_volume().await?.share().id().clone()
            }
            Err(error) => return Err(error.into()),
        };

        let share = self.open_share(&share).await?;
        let root = self.root(&share).await?;

        Ok(PhotoLibrary { share, root })
    }

    pub fn upload_photo<'a>(
        &'a self,
        library: &'a PhotoLibrary,
        name: impl Into<String>,
    ) -> Upload<'a> {
        self.upload(library.share(), library.root(), name)
            .photo(PhotoDetails::new())
    }

    async fn create_photo_volume(&self) -> Result<Volume> {
        let client = self.clone();
        let bootstrap = offload(move || client.bootstrap(ROOT_NAME)).await?;
        let request = CreatePhotoVolumeRequest {
            share: bootstrap.share,
            link: bootstrap.root,
        };
        Ok(self
            .api()
            .post("drive/photos/volumes")
            .json(&request)
            .send_field("Volume")
            .await?)
    }
}

#[derive(Debug, Serialize)]
struct CreatePhotoVolumeRequest {
    #[serde(rename = "Share")]
    share: ShareBootstrap,
    #[serde(rename = "Link")]
    link: RootBootstrap,
}
