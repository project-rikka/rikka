use std::io::{Read, Seek, Write};

use sevenz_rust2::encoder_options::{AesEncoderOptions, EncoderOptions, Lzma2Options};
use sevenz_rust2::{ArchiveEntry, EncoderConfiguration, EncoderMethod, NtTime};

use crate::error::Result;
use crate::level::Level;
use crate::mode;
use crate::writer::options::CreateOptions;
use crate::writer::{NewEntry, WriterBackend};

const MT_CHUNK_SIZE: u64 = 64 * 1024 * 1024;

pub(crate) struct SevenZWriterBackend<W: Write + Seek> {
    writer: sevenz_rust2::ArchiveWriter<W>,
}

impl<W: Write + Seek> SevenZWriterBackend<W> {
    pub(crate) fn new(sink: W, options: &CreateOptions<'_>) -> Result<SevenZWriterBackend<W>> {
        let mut writer = sevenz_rust2::ArchiveWriter::new(sink)?;

        let mut methods = Vec::with_capacity(2);
        if let Some(password) = &options.password {
            methods.push(AesEncoderOptions::new(password.to_seven_z()).into());
            writer.set_encrypt_header(options.encrypt_header);
        } else {
            writer.set_encrypt_header(false);
        }
        methods.push(content_method(options.level, options.thread_count()));
        writer.set_content_methods(methods);

        Ok(SevenZWriterBackend { writer })
    }

    fn push(&mut self, entry: ArchiveEntry, data: Option<&mut dyn Read>) -> Result<()> {
        self.writer.push_archive_entry(entry, data)?;
        Ok(())
    }
}

impl<W: Write + Seek> WriterBackend for SevenZWriterBackend<W> {
    fn add_file(&mut self, entry: &NewEntry, data: &mut dyn Read) -> Result<()> {
        self.push(archive_entry(entry, mode::S_IFREG, false), Some(data))
    }

    fn add_directory(&mut self, entry: &NewEntry) -> Result<()> {
        self.push(archive_entry(entry, mode::S_IFDIR, true), None)
    }

    fn add_symlink(&mut self, entry: &NewEntry, target: &str) -> Result<()> {
        let mut bytes = target.as_bytes();
        self.push(archive_entry(entry, mode::S_IFLNK, false), Some(&mut bytes))
    }

    fn finish(self: Box<Self>) -> Result<()> {
        let mut sink = self.writer.finish()?;
        sink.flush()?;
        Ok(())
    }
}

fn archive_entry(entry: &NewEntry, file_type: u32, directory: bool) -> ArchiveEntry {
    let name = entry.name.trim_end_matches('/').to_owned();
    let mut archive_entry = if directory {
        ArchiveEntry::new_directory(&name)
    } else {
        ArchiveEntry::new_file(&name)
    };

    if let Some(modified) = entry.modified.and_then(|time| NtTime::try_from(time).ok()) {
        archive_entry.has_last_modified_date = true;
        archive_entry.last_modified_date = modified;
    }

    let permissions = entry.mode.unwrap_or(if directory { 0o755 } else { 0o644 });
    archive_entry.has_windows_attributes = true;
    archive_entry.windows_attributes =
        mode::to_windows_attributes(permissions, file_type, directory);

    archive_entry
}

fn content_method(level: Level, threads: u32) -> EncoderConfiguration {
    if level.is_store() {
        return EncoderConfiguration::new(EncoderMethod::COPY);
    }

    let options = if threads > 1 {
        Lzma2Options::from_level_mt(level.preset(), threads, MT_CHUNK_SIZE)
    } else {
        Lzma2Options::from_level(level.preset())
    };
    EncoderConfiguration::new(EncoderMethod::LZMA2).with_options(EncoderOptions::Lzma2(options))
}
