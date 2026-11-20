use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use md5::{Digest, Md5};
use rikka_image::Image;
use rikka_types::Size as Pixels;

use crate::png;
use crate::size::Size;

const SOFTWARE: &str = "Rikka";

const URI: &str = "Thumb::URI";

const MTIME: &str = "Thumb::MTime";

const BYTES: &str = "Thumb::Size";

#[derive(Clone, Debug)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    #[must_use]
    pub fn shared() -> Option<Self> {
        Some(Self {
            root: rikka_dirs::cache_home()?.join("thumbnails"),
        })
    }

    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn path_of(&self, uri: &str, size: Size) -> PathBuf {
        self.root.join(size.directory()).join(name_of(uri))
    }

    #[must_use]
    pub fn failure_path(&self, uri: &str, application: &str) -> PathBuf {
        self.root.join("fail").join(application).join(name_of(uri))
    }

    #[must_use]
    pub fn holds(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }

    #[must_use]
    pub fn describing(thumbnail: &Path, uri: &str, modified: SystemTime) -> Option<Pixels> {
        let contents = png::read(thumbnail)?;
        let field = |name: &str| {
            contents
                .text
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        let describes =
            field(URI)? == uri && field(MTIME)?.parse::<i64>().ok() == Some(seconds(modified));
        describes.then_some(contents.pixels)
    }

    #[must_use]
    pub fn fields(uri: &str, modified: SystemTime, bytes: Option<u64>) -> Vec<(String, String)> {
        let mut fields = vec![
            (URI.to_owned(), uri.to_owned()),
            (MTIME.to_owned(), seconds(modified).to_string()),
            ("Software".to_owned(), SOFTWARE.to_owned()),
        ];
        if let Some(bytes) = bytes {
            fields.push((BYTES.to_owned(), bytes.to_string()));
        }
        fields
    }

    pub fn prepare(&self, thumbnail: &Path) -> std::io::Result<()> {
        let Some(directory) = thumbnail.parent() else {
            return Ok(());
        };
        std::fs::create_dir_all(directory)?;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
    }

    pub fn store(
        &self,
        thumbnail: &Path,
        picture: Image,
        fields: &[(String, String)],
    ) -> std::io::Result<()> {
        self.prepare(thumbnail)?;
        png::write(thumbnail, picture, fields)
    }
}

fn name_of(uri: &str) -> String {
    let digest = Md5::digest(uri.as_bytes());
    let mut name = rikka_types::hex::encode(digest);
    name.push_str(".png");
    name
}

fn seconds(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        Err(before) => -i64::try_from(before.duration().as_secs()).unwrap_or(i64::MAX),
    }
}
