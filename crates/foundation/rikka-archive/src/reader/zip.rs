use std::io::{Read, Seek};

use zip::read::{ZipFile, ZipReadOptions};

use crate::compression::Compression;
use crate::encoding;
use crate::entry::Entry;
use crate::error::Result;
use crate::mode;
use crate::password::Password;
use crate::path;
use crate::reader::{EntrySink, ReaderBackend};
use crate::timestamp;

pub(crate) struct ZipReader<R: Read + Seek> {
    archive: zip::ZipArchive<R>,
    entries: Vec<Entry>,
}

impl<R: Read + Seek> ZipReader<R> {
    pub(crate) fn new(source: R) -> Result<ZipReader<R>> {
        let mut archive = zip::ZipArchive::new(source)?;

        let mut entries = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            entries.push(read_entry(&archive.by_index_raw(index)?));
        }

        Ok(ZipReader { archive, entries })
    }
}

impl<R: Read + Seek> ReaderBackend for ZipReader<R> {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn read_entries(
        &mut self,
        indices: &[usize],
        password: &Password,
        each: &mut EntrySink<'_>,
    ) -> Result<()> {
        for &index in indices {
            let options =
                ZipReadOptions::new().password((!password.is_empty()).then(|| password.as_bytes()));
            let mut file = self.archive.by_index_with_options(index, options)?;
            each(index, &mut file)?;
        }
        Ok(())
    }

    fn comment(&self) -> Option<String> {
        let comment = self.archive.comment();
        (!comment.is_empty()).then(|| encoding::decode_name(comment))
    }
}

fn read_entry<R: Read + ?Sized>(file: &ZipFile<'_, R>) -> Entry {
    let name = path::normalize(&encoding::decode_name(file.name_raw()));
    let unix_mode = file.unix_mode();

    let mut entry = Entry::new(name, mode::kind_of(unix_mode, file.is_dir()));
    entry.size = file.size();
    entry.compressed_size = Some(file.compressed_size());
    entry.modified = file.last_modified().and_then(timestamp::from_dos);
    entry.mode = unix_mode;
    entry.crc32 = Some(file.crc32());
    entry.encrypted = file.encrypted();
    entry.compression = Some(Compression::from_zip(file.compression()));
    entry
}
