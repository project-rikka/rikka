use bytes::Bytes;
use rikka_proton_api::Multipart;
use rikka_proton_pgp::{Pgp, SessionKey};

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::revision::Block;

pub(crate) const STORAGE_TOKEN: &str = "pm-storage-token";
pub(crate) const BLOCK_FIELD: &str = "Block";
pub(crate) const BLOCK_FILE_NAME: &str = "blob";

pub(crate) struct DecryptedBlock {
    pub(crate) digest: [u8; 32],
    pub(crate) plaintext: Vec<u8>,
}

impl DecryptedBlock {
    pub(crate) fn decrypt(
        pgp: &Pgp,
        content_key: &SessionKey,
        block: &Block,
        ciphertext: &[u8],
    ) -> Result<DecryptedBlock> {
        let digest = crypto::digest(ciphertext);
        if let Some(hash) = block.hash()
            && rikka_proton_api::base64::encode(digest) != hash
        {
            return Err(Error::BlockCorrupted {
                index: block.index(),
            });
        }

        Ok(DecryptedBlock {
            digest,
            plaintext: crypto::decrypt_block(pgp, content_key, ciphertext)?,
        })
    }
}

impl DriveClient {
    pub(crate) async fn fetch_block(&self, block: &Block) -> Result<Bytes> {
        let index = block.index();
        let (url, token) = match (block.bare_url(), block.token()) {
            (Some(url), Some(token)) => (url, Some(token)),
            _ => (
                block.url().ok_or(Error::BlockUnreachable { index })?,
                block.token(),
            ),
        };

        self.fetch_stored(url, token).await
    }

    pub(crate) async fn fetch_stored(&self, url: &str, token: Option<&str>) -> Result<Bytes> {
        let mut request = self.api().http().get(url);
        if let Some(token) = token {
            request = request.header(STORAGE_TOKEN, token);
        }

        let response = request.send().await?.error_for_status().await?;

        Ok(response.bytes().await?)
    }

    pub(crate) async fn write_block(&self, url: &str, token: &str, data: Vec<u8>) -> Result<()> {
        self.api()
            .post(url)
            .anonymous()
            .header(STORAGE_TOKEN, token)
            .multipart(Multipart::new().file(BLOCK_FIELD, BLOCK_FILE_NAME, data))
            .send_bytes()
            .await
            .map_err(|error| Error::Transfer(Box::new(error)))?;

        Ok(())
    }
}
