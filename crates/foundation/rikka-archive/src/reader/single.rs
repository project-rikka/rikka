use std::io::{Read, Seek, SeekFrom};

use crate::entry::{Entry, EntryKind};
use crate::error::Result;
use crate::password::Password;
use crate::reader::{EntrySink, ReaderBackend};
use crate::stream::StreamCompression;

pub(crate) struct SingleReader<R: Read + Seek> {
    source: R,
    compression: StreamCompression,
    entries: Vec<Entry>,
}

impl<R: Read + Seek> SingleReader<R> {
    pub(crate) fn new(
        mut source: R,
        compression: StreamCompression,
        name_hint: Option<String>,
    ) -> Result<SingleReader<R>> {
        let length = source.seek(SeekFrom::End(0))?;
        let metadata = compression.read_metadata(&mut source, length)?;

        let name = metadata
            .name
            .or(name_hint)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "data".to_owned());

        let mut entry = Entry::new(name, EntryKind::File);
        entry.modified = metadata.modified;
        entry.size = metadata.size.unwrap_or(0);
        entry.compressed_size = Some(length);

        Ok(SingleReader {
            source,
            compression,
            entries: vec![entry],
        })
    }
}

impl<R: Read + Seek> ReaderBackend for SingleReader<R> {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn read_entries(
        &mut self,
        indices: &[usize],
        _password: &Password,
        each: &mut EntrySink<'_>,
    ) -> Result<()> {
        if indices.is_empty() {
            return Ok(());
        }

        self.source.rewind()?;
        let mut decoder = self.compression.decoder(&mut self.source)?;
        each(0, &mut *decoder)
    }
}
