use std::io::{Read, Seek, Write};

use zip::AesMode;
use zip::write::FileOptions;

use crate::error::Result;
use crate::level::Level;
use crate::mode;
use crate::password::Password;
use crate::timestamp;
use crate::writer::options::CreateOptions;
use crate::writer::{NewEntry, WriterBackend};

pub(crate) struct ZipWriterBackend<W: Write + Seek> {
    writer: zip::ZipWriter<W>,
    level: Level,
    password: Option<Password>,
}

impl<W: Write + Seek> ZipWriterBackend<W> {
    pub(crate) fn new(sink: W, options: &CreateOptions<'_>) -> ZipWriterBackend<W> {
        ZipWriterBackend {
            writer: zip::ZipWriter::new(sink).set_auto_large_file(),
            level: options.level,
            password: options.password.clone(),
        }
    }
}

impl<W: Write + Seek> WriterBackend for ZipWriterBackend<W> {
    fn add_file(&mut self, entry: &NewEntry, data: &mut dyn Read) -> Result<()> {
        let mut options = file_options(entry, self.level);
        if let Some(password) = self.password.as_ref() {
            options = options.with_aes_encryption_bytes(AesMode::Aes256, password.as_bytes());
        }

        self.writer.start_file(&entry.name, options)?;
        std::io::copy(data, &mut self.writer)?;
        Ok(())
    }

    fn add_directory(&mut self, entry: &NewEntry) -> Result<()> {
        self.writer.add_directory(
            entry.name.trim_end_matches('/'),
            file_options(entry, self.level),
        )?;
        Ok(())
    }

    fn add_symlink(&mut self, entry: &NewEntry, target: &str) -> Result<()> {
        self.writer
            .add_symlink(&entry.name, target, file_options(entry, self.level))?;
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<()> {
        let mut sink = self.writer.finish()?;
        sink.flush()?;
        Ok(())
    }
}

fn file_options<'a>(entry: &NewEntry, level: Level) -> FileOptions<'a, ()> {
    let mut options = FileOptions::default();
    options = if level.is_store() {
        options.compression_method(zip::CompressionMethod::Stored)
    } else {
        options
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(i64::from(level.scale())))
    };

    if let Some(mode) = entry.mode {
        options = options.unix_permissions(mode & mode::PERMISSION_BITS);
    }
    if let Some(modified) = entry.modified.and_then(timestamp::to_dos) {
        options = options.last_modified_time(modified);
    }
    options
}
