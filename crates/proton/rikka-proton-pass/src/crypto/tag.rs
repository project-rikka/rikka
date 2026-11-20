use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EncryptionTag {
    VaultContent,
    ItemKey,
    ItemContent,
    LinkKey,
    FileKey,
    FileData,
    FileMetadata,
    FileChunk { index: usize, chunks: usize },
    FolderKey,
    FolderContent,
}

impl EncryptionTag {
    #[must_use]
    pub fn aad(&self) -> Cow<'static, [u8]> {
        match self {
            EncryptionTag::VaultContent => Cow::Borrowed(b"vaultcontent"),
            EncryptionTag::ItemKey => Cow::Borrowed(b"itemkey"),
            EncryptionTag::ItemContent => Cow::Borrowed(b"itemcontent"),
            EncryptionTag::LinkKey => Cow::Borrowed(b"linkkey"),
            EncryptionTag::FileKey => Cow::Borrowed(b"filekey"),
            EncryptionTag::FileData => Cow::Borrowed(b"filedata"),
            EncryptionTag::FileMetadata => Cow::Borrowed(b"v2;filemetadata.item.pass.proton"),
            EncryptionTag::FileChunk { index, chunks } => {
                Cow::Owned(format!("v2;{index};{chunks};filedata.item.pass.proton").into_bytes())
            }
            EncryptionTag::FolderKey => Cow::Borrowed(b"key.folder.pass.proton"),
            EncryptionTag::FolderContent => Cow::Borrowed(b"content.folder.pass.proton"),
        }
    }
}
