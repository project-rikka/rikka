use std::path::Path;

use crate::entry::Entry;
use crate::error::Result;
use crate::extract::{ExtractOptions, ExtractSummary};
use crate::reader::Archive;
use crate::writer::{ArchiveWriter, CreateOptions};

pub fn extract(archive: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<ExtractSummary> {
    extract_with(archive, destination, ExtractOptions::new())
}

pub fn extract_with(
    archive: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    options: ExtractOptions<'_>,
) -> Result<ExtractSummary> {
    let password = options.password.clone();
    let mut archive = match password {
        Some(password) => Archive::open_with_password(archive, password)?,
        None => Archive::open(archive)?,
    };
    archive.extract_with(destination, options)
}

pub fn list(archive: impl AsRef<Path>) -> Result<Vec<Entry>> {
    Ok(Archive::open(archive)?.entries().to_vec())
}

pub fn compress(
    archive: impl AsRef<Path>,
    sources: impl IntoIterator<Item = impl AsRef<Path>>,
) -> Result<()> {
    let mut writer = ArchiveWriter::create_with(archive, CreateOptions::new())?;
    for source in sources {
        writer.add_path(source)?;
    }
    writer.finish()
}
