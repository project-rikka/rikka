use rikka_proton_api::Api;

use crate::api::{FileResponse, SealedLinkContent};
use crate::client::PREFIX;
use crate::crypto::EncryptionKey;
use crate::download::{ChunkRoute, Download};
use crate::error::Result;
use crate::secure_link_content::SecureLinkContent;
use crate::shared_file::SharedFile;

#[derive(Debug, Clone)]
pub struct SecureLinkViewer {
    api: Api,
}

impl SecureLinkViewer {
    #[must_use]
    pub fn new(api: Api) -> SecureLinkViewer {
        SecureLinkViewer { api }
    }

    pub async fn open(&self, link: &str) -> Result<SecureLinkContent> {
        let (token, link_key) = crate::crypto::secure_link::parse_link_url(link)?;

        let content: SealedLinkContent = self
            .api
            .get(format!("{PREFIX}/public_link/content/{token}"))
            .send_field("PublicLinkContent")
            .await?;

        let files = match &content.files_token {
            Some(files) => {
                let files: Option<Vec<FileResponse>> = self
                    .api
                    .get(format!("{PREFIX}/public_link/files/{files}"))
                    .send_field_at(&["Files", "Files"])
                    .await?;
                files.unwrap_or_default()
            }
            None => Vec::new(),
        };

        rikka_proton_api::offload(move || {
            let item_key =
                crate::crypto::secure_link::decrypt_item_key(&link_key, &content.item_key)?;
            let item =
                crate::crypto::secure_link::decrypt_item_content(&item_key, &content.contents)?;
            let token = content.files_token.unwrap_or_default();
            let files = files
                .into_iter()
                .map(|file| shared_file(&token, &item_key, file))
                .collect::<Result<Vec<SharedFile>>>()?;

            Ok(SecureLinkContent {
                item,
                content_format_version: content.content_format_version,
                read_count: content.read_count,
                max_read_count: content.max_read_count,
                expiration_time: content.expiration_time,
                files,
            })
        })
        .await
    }

    pub fn download_file(&self, file: &SharedFile) -> Download {
        Download::new(
            self.api.clone(),
            ChunkRoute::Public {
                token: file.token().to_owned(),
                file: file.id().clone(),
            },
            file.key().clone(),
            file.response().encryption_version(),
            file.chunks(),
        )
    }
}

fn shared_file(token: &str, item_key: &EncryptionKey, file: FileResponse) -> Result<SharedFile> {
    let key = crate::crypto::file::decrypt_file_key(item_key, file.file_key())?;
    let metadata =
        crate::crypto::file::decrypt_metadata(&key, file.metadata(), file.encryption_version())?;

    Ok(SharedFile::new(token.to_owned(), file, metadata, key))
}
