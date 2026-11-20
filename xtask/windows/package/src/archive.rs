use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::entries::{Entries, Mode};

const COMPRESSION_LEVEL: i64 = 9;
const EXECUTABLE_PERMISSIONS: u32 = 0o755;
const DATA_PERMISSIONS: u32 = 0o644;

pub(crate) fn write(entries: &Entries, output: &Path) -> Result<()> {
    let file =
        File::create(output).with_context(|| format!("failed to create {}", output.display()))?;
    let mut archive = ZipWriter::new(BufWriter::new(file));
    for entry in entries.iter() {
        let mut source = File::open(&entry.source)
            .with_context(|| format!("failed to open {}", entry.source.display()))?;
        let length = source
            .metadata()
            .with_context(|| format!("failed to read {}", entry.source.display()))?
            .len();
        let permissions = match entry.mode {
            Mode::Executable => EXECUTABLE_PERMISSIONS,
            Mode::Data => DATA_PERMISSIONS,
        };
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(COMPRESSION_LEVEL))
            .last_modified_time(DateTime::default())
            .unix_permissions(permissions)
            .large_file(length > u64::from(u32::MAX));
        archive
            .start_file(entry.name.as_str(), options)
            .with_context(|| format!("failed to add {}", entry.name))?;
        std::io::copy(&mut source, &mut archive)
            .with_context(|| format!("failed to pack {}", entry.source.display()))?;
    }
    archive
        .finish()
        .context("failed to finish the archive")?
        .flush()
        .with_context(|| format!("failed to write {}", output.display()))
}
