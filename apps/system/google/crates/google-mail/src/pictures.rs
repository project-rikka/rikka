use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const KINDS: [(&[u8], &str); 5] = [
    (b"\x89PNG\r\n\x1a\n", "png"),
    (b"\xff\xd8\xff", "jpg"),
    (b"GIF8", "gif"),
    (b"RIFF", "webp"),
    (b"BM", "bmp"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pictures {
    folder: PathBuf,
}

impl Pictures {
    #[must_use]
    pub fn new(folder: impl Into<PathBuf>) -> Pictures {
        Pictures {
            folder: folder.into(),
        }
    }

    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    #[must_use]
    pub fn kept(&self, key: &str) -> Option<String> {
        let stem = stem(key);
        KINDS
            .iter()
            .map(|(_, extension)| format!("{stem}.{extension}"))
            .find(|name| self.folder.join(name).is_file())
    }

    pub fn keep(&self, key: &str, picture: &[u8]) -> std::io::Result<Option<String>> {
        let Some((_, extension)) = KINDS
            .iter()
            .find(|(signature, _)| picture.starts_with(signature))
        else {
            return Ok(None);
        };
        std::fs::create_dir_all(&self.folder)?;
        let name = format!("{}.{extension}", stem(key));
        rikka_fs::Replace::new(self.folder.join(&name))
            .private()
            .unsynced()
            .write(picture)?;
        Ok(Some(name))
    }
}

fn stem(key: &str) -> String {
    rikka_types::hex::encode(&Sha256::digest(key.as_bytes())[..16])
}
