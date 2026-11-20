use std::io::{Read, Seek, SeekFrom};

use tar::EntryType;

use crate::encoding;
use crate::entry::{Entry, EntryKind};
use crate::error::Result;
use crate::password::Password;
use crate::path;
use crate::reader::{EntrySink, ReaderBackend};
use crate::stream::StreamCompression;
use crate::timestamp;

pub(crate) struct TarReader<R: Read + Seek> {
    source: R,
    compression: Option<StreamCompression>,
    entries: Vec<Entry>,
    positions: Vec<u64>,
}

impl<R: Read + Seek> TarReader<R> {
    pub(crate) fn new(
        mut source: R,
        compression: Option<StreamCompression>,
    ) -> Result<TarReader<R>> {
        let mut entries = Vec::new();
        let mut positions = Vec::new();

        {
            let stream = open_stream(&mut source, compression)?;
            let mut archive = tar::Archive::new(stream);
            for entry in archive.entries()? {
                let entry = entry?;
                positions.push(entry.raw_file_position());
                entries.push(read_entry(&entry));
            }
        }

        Ok(TarReader {
            source,
            compression,
            entries,
            positions,
        })
    }
}

impl<R: Read + Seek> ReaderBackend for TarReader<R> {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn read_entries(
        &mut self,
        indices: &[usize],
        _password: &Password,
        each: &mut EntrySink<'_>,
    ) -> Result<()> {
        if self.compression.is_none() {
            for &index in indices {
                self.source.seek(SeekFrom::Start(self.positions[index]))?;
                let mut data = (&mut self.source).take(self.entries[index].size);
                each(index, &mut data)?;
            }
            return Ok(());
        }

        let stream = open_stream(&mut self.source, self.compression)?;
        let mut archive = tar::Archive::new(stream);
        let mut remaining = indices.iter().copied().peekable();

        for (position, entry) in archive.entries()?.enumerate() {
            let mut entry = entry?;
            let Some(&next) = remaining.peek() else {
                break;
            };
            if position != next {
                continue;
            }
            remaining.next();
            each(position, &mut entry)?;
        }
        Ok(())
    }
}

fn open_stream<'a, R: Read + Seek>(
    source: &'a mut R,
    compression: Option<StreamCompression>,
) -> Result<Box<dyn Read + 'a>> {
    source.rewind()?;
    match compression {
        Some(compression) => compression.decoder(source),
        None => Ok(Box::new(source)),
    }
}

fn read_entry<R: Read>(entry: &tar::Entry<'_, R>) -> Entry {
    let header = entry.header();

    let kind = match header.entry_type() {
        EntryType::Directory => EntryKind::Directory,
        EntryType::Symlink => EntryKind::Symlink,
        EntryType::Link => EntryKind::HardLink,
        EntryType::Regular | EntryType::Continuous | EntryType::GNUSparse => EntryKind::File,
        _ => EntryKind::Special,
    };

    let mut archive_entry = Entry::new(
        path::normalize(&encoding::decode_name(&entry.path_bytes())),
        kind,
    );
    archive_entry.link_target = entry
        .link_name_bytes()
        .map(|target| encoding::decode_name(&target));
    archive_entry.size = entry.size();
    archive_entry.mode = header.mode().ok();
    archive_entry.modified = header
        .mtime()
        .ok()
        .and_then(|seconds| i64::try_from(seconds).ok())
        .map(timestamp::from_unix_seconds);
    archive_entry
}
