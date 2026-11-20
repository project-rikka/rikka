use std::io::{Read, Write};

use tar::{EntryType, Header};

use crate::error::Result;
use crate::mode;
use crate::stream::{EncoderStream, Passthrough, StreamCompression};
use crate::timestamp;
use crate::writer::options::CreateOptions;
use crate::writer::{NewEntry, WriterBackend};

pub(crate) struct TarWriterBackend {
    builder: tar::Builder<Box<dyn EncoderStream>>,
}

impl TarWriterBackend {
    pub(crate) fn new<W: Write + 'static>(
        sink: W,
        compression: Option<StreamCompression>,
        options: &CreateOptions<'_>,
    ) -> Result<TarWriterBackend> {
        let stream: Box<dyn EncoderStream> = match compression {
            Some(compression) => compression.encoder(sink, options.level)?,
            None => Box::new(Passthrough::new(sink)),
        };

        let mut builder = tar::Builder::new(stream);
        builder.follow_symlinks(false);

        Ok(TarWriterBackend { builder })
    }
}

impl WriterBackend for TarWriterBackend {
    fn add_file(&mut self, entry: &NewEntry, data: &mut dyn Read) -> Result<()> {
        let mut header = header_for(entry, EntryType::Regular, 0o644);
        header.set_size(entry.size);
        self.builder.append_data(&mut header, &entry.name, data)?;
        Ok(())
    }

    fn add_directory(&mut self, entry: &NewEntry) -> Result<()> {
        let mut header = header_for(entry, EntryType::Directory, 0o755);
        header.set_size(0);
        self.builder
            .append_data(&mut header, &entry.name, std::io::empty())?;
        Ok(())
    }

    fn add_symlink(&mut self, entry: &NewEntry, target: &str) -> Result<()> {
        let mut header = header_for(entry, EntryType::Symlink, 0o777);
        header.set_size(0);
        self.builder.append_link(&mut header, &entry.name, target)?;
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<()> {
        let stream = self.builder.into_inner()?;
        stream.finish_stream()
    }
}

fn header_for(entry: &NewEntry, entry_type: EntryType, default_mode: u32) -> Header {
    let mut header = Header::new_gnu();
    header.set_entry_type(entry_type);
    header.set_mode(entry.mode.unwrap_or(default_mode) & mode::PERMISSION_BITS);
    header.set_mtime(
        entry
            .modified
            .map(timestamp::to_unix_seconds)
            .and_then(|seconds| u64::try_from(seconds).ok())
            .unwrap_or(0),
    );
    header
}
