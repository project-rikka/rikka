use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use rikka_media_format::Extent;

use crate::catalog;
use crate::format::Format;
use crate::kind::Kind;

const HEADER: u64 = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Media {
    pub format: Format,
    pub extent: Option<Extent>,
    pub bytes: u64,
}

impl Media {
    #[must_use]
    pub fn kind(self) -> Kind {
        self.format.kind()
    }
}

pub fn probe(path: &Path) -> io::Result<Option<Media>> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.is_dir() {
        return Err(io::Error::from(io::ErrorKind::IsADirectory));
    }
    let bytes = metadata.len();

    let mut header = Vec::new();
    file.take(HEADER).read_to_end(&mut header)?;

    let Some(format) = catalog::identify(path, &header) else {
        return Ok(crate::textual::is_probably_text(&header).then_some(Media {
            format: catalog::TEXT,
            extent: None,
            bytes,
        }));
    };

    Ok(Some(Media {
        format,
        extent: format.extent(&header),
        bytes,
    }))
}
