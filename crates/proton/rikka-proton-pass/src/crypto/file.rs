use crate::content::FileMetadata;
use crate::crypto::key::EncryptionKey;
use crate::crypto::tag::EncryptionTag;
use crate::crypto::{decode_base64, encode_base64};
use crate::error::{Error, Result};

pub(crate) const ENCRYPTION_VERSION: i32 = 2;

fn metadata_tag(version: i32) -> Result<EncryptionTag> {
    match version {
        1 => Ok(EncryptionTag::FileData),
        2 => Ok(EncryptionTag::FileMetadata),
        version => Err(Error::UnsupportedFileVersion { version }),
    }
}

fn chunk_tag(version: i32, index: usize, chunks: usize) -> Result<EncryptionTag> {
    match version {
        1 => Ok(EncryptionTag::FileData),
        2 => Ok(EncryptionTag::FileChunk { index, chunks }),
        version => Err(Error::UnsupportedFileVersion { version }),
    }
}

pub(crate) fn encrypt_metadata(
    key: &EncryptionKey,
    metadata: &FileMetadata,
    version: i32,
) -> Result<String> {
    let sealed = key.seal(metadata.encode(), &metadata_tag(version)?)?;

    Ok(encode_base64(sealed))
}

pub(crate) fn decrypt_metadata(
    key: &EncryptionKey,
    sealed: &str,
    version: i32,
) -> Result<FileMetadata> {
    let sealed = decode_base64("file metadata", sealed)?;
    let decrypted = key.open(sealed, &metadata_tag(version)?)?;

    FileMetadata::decode(&decrypted)
}

pub(crate) fn encrypt_file_key(
    item_key: &EncryptionKey,
    file_key: &EncryptionKey,
) -> Result<String> {
    let sealed = item_key.seal(file_key.as_bytes(), &EncryptionTag::FileKey)?;

    Ok(encode_base64(sealed))
}

pub(crate) fn decrypt_file_key(item_key: &EncryptionKey, sealed: &str) -> Result<EncryptionKey> {
    let sealed = decode_base64("file key", sealed)?;
    let decrypted = item_key.open(sealed, &EncryptionTag::FileKey)?;

    EncryptionKey::from_bytes(decrypted)
}

pub(crate) fn encrypt_chunk(
    key: &EncryptionKey,
    data: &[u8],
    index: usize,
    chunks: usize,
    version: i32,
) -> Result<Vec<u8>> {
    key.seal(data, &chunk_tag(version, index, chunks)?)
}

pub(crate) fn decrypt_chunk(
    key: &EncryptionKey,
    data: &[u8],
    index: usize,
    chunks: usize,
    version: i32,
) -> Result<Vec<u8>> {
    key.open(data, &chunk_tag(version, index, chunks)?)
}
