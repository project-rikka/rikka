use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use rikka_image::{Filter, Jpeg};
use sha2::{Digest, Sha256};

use crate::model::{Artwork, ArtworkFile};

const VARIANTS: [u32; 2] = [300, 600];
const HASH_BYTES: usize = 16;
const VARIANT_QUALITY: u8 = 88;
const VARIANT_EXTENSION: &str = "jpg";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stored {
    pub(crate) hash: String,
    pub(crate) extension: String,
    pub(crate) width: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ArtworkCache {
    folder: PathBuf,
}

impl ArtworkCache {
    pub(crate) fn new(folder: PathBuf) -> ArtworkCache {
        ArtworkCache { folder }
    }

    pub(crate) fn store(&self, bytes: &[u8]) -> Option<Stored> {
        let info = rikka_image::read(bytes).probe().ok()?;
        if info.width == 0 || info.height == 0 {
            return None;
        }
        let digest = Sha256::digest(bytes);
        let hash = rikka_types::hex::encode(&digest[..HASH_BYTES]);
        let stored = Stored {
            hash,
            extension: info.format.extension().to_owned(),
            width: info.width,
        };
        let original = self.original(&stored.hash, &stored.extension);
        if original.exists() {
            return Some(stored);
        }
        if let Err(error) = fs::create_dir_all(&self.folder) {
            tracing::warn!("failed to create artwork folder: {error}");
            return None;
        }
        let image = rikka_image::read(bytes).decode().ok()?;
        for size in VARIANTS.into_iter().filter(|size| *size < info.width) {
            let Ok(variant) = image.resize_to_fit(size, size, Filter::Lanczos3) else {
                continue;
            };
            let target = self.variant(&stored.hash, size);
            let stored = variant
                .write(Jpeg)
                .quality(VARIANT_QUALITY)
                .to_bytes()
                .map_err(|error| error.to_string())
                .and_then(|encoded| {
                    rikka_fs::replace(&target, encoded).map_err(|error| error.to_string())
                });
            if let Err(error) = stored {
                tracing::warn!("failed to write artwork variant: {error}");
            }
        }
        if let Err(error) = rikka_fs::replace(&original, bytes) {
            tracing::warn!("failed to store artwork: {error}");
            return None;
        }
        Some(stored)
    }

    pub(crate) fn artwork(&self, hash: &str, extension: &str, width: u32) -> Artwork {
        let mut files: Vec<ArtworkFile> = VARIANTS
            .into_iter()
            .filter(|size| *size < width)
            .map(|size| ArtworkFile {
                path: self.variant(hash, size),
                width: size,
            })
            .collect();
        files.push(ArtworkFile {
            path: self.original(hash, extension),
            width,
        });
        Artwork { files }
    }

    pub(crate) fn forget(&self, hash: &str, extension: &str) {
        let paths = VARIANTS
            .into_iter()
            .map(|size| self.variant(hash, size))
            .chain(std::iter::once(self.original(hash, extension)));
        for path in paths {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => tracing::warn!("failed to remove {}: {error}", path.display()),
            }
        }
    }

    fn original(&self, hash: &str, extension: &str) -> PathBuf {
        self.folder.join(format!("{hash}.{extension}"))
    }

    fn variant(&self, hash: &str, size: u32) -> PathBuf {
        self.folder
            .join(format!("{hash}-{size}.{VARIANT_EXTENSION}"))
    }
}
