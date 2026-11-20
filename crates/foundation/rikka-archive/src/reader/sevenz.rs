use std::io::{Read, Seek};

use sevenz_rust2::{Archive as SevenZArchive, BlockDecoder, EncoderMethod};

use crate::compression::Compression;
use crate::entry::Entry;
use crate::error::{Error, Result};
use crate::mode;
use crate::password::Password;
use crate::path;
use crate::reader::verify::{CrcReader, Integrity};
use crate::reader::{EntrySink, ReaderBackend};
use crate::threads;

#[derive(Clone, Copy)]
struct BlockCoding {
    encrypted: bool,
    compression: Compression,
}

pub(crate) struct SevenZReader<R: Read + Seek> {
    archive: SevenZArchive,
    source: R,
    coding: Vec<BlockCoding>,
    entries: Vec<Entry>,
    threads: u32,
}

impl<R: Read + Seek> SevenZReader<R> {
    pub(crate) fn new(mut source: R, password: &Password) -> Result<SevenZReader<R>> {
        let archive = SevenZArchive::read(&mut source, &password.to_seven_z())?;

        let coding: Vec<BlockCoding> = archive
            .blocks
            .iter()
            .map(|block| {
                let methods: Vec<EncoderMethod> = block
                    .coders
                    .iter()
                    .filter_map(|coder| EncoderMethod::by_id(coder.encoder_method_id()))
                    .collect();
                BlockCoding {
                    encrypted: methods.contains(&EncoderMethod::AES256_SHA256),
                    compression: Compression::from_seven_z(&methods),
                }
            })
            .collect();

        let entries = archive
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| {
                let block = archive.stream_map.file_block_index[index]
                    .and_then(|block| coding.get(block).copied());
                read_entry(file, block)
            })
            .collect();

        Ok(SevenZReader {
            archive,
            source,
            coding,
            entries,
            threads: threads::available(),
        })
    }
}

impl<R: Read + Seek> ReaderBackend for SevenZReader<R> {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn read_entries(
        &mut self,
        indices: &[usize],
        password: &Password,
        each: &mut EntrySink<'_>,
    ) -> Result<()> {
        let supplied_password = !password.is_empty();
        let password = password.to_seven_z();
        let threads = self.threads;
        let mut remaining = indices;

        while let Some(&first) = remaining.first() {
            let Some(block_index) = self.archive.stream_map.file_block_index[first] else {
                each(first, &mut std::io::empty())?;
                remaining = &remaining[1..];
                continue;
            };

            let batch_len = remaining
                .iter()
                .take_while(|&&index| {
                    self.archive.stream_map.file_block_index.get(index).copied()
                        == Some(Some(block_index))
                })
                .count();
            let (batch, rest) = remaining.split_at(batch_len);
            remaining = rest;

            let decrypting = supplied_password
                && self
                    .coding
                    .get(block_index)
                    .is_some_and(|coding| coding.encrypted);

            let mut remaining = batch.iter().copied().peekable();
            let mut position = self.archive.stream_map.block_first_file_index[block_index];
            let mut failure = None;

            let decoder = BlockDecoder::new(
                threads,
                block_index,
                &self.archive,
                &password,
                &mut self.source,
            );
            decoder.for_each_entries(&mut |file, reader| {
                let index = position;
                position += 1;

                match remaining.peek() {
                    Some(&next) if next == index => {
                        remaining.next();
                        if let Err(error) = read_verified(index, file, reader, decrypting, each) {
                            failure = Some(error);
                            return Ok(false);
                        }
                        Ok(remaining.peek().is_some())
                    }
                    Some(_) => {
                        std::io::copy(reader, &mut std::io::sink()).map_err(|error| {
                            sevenz_rust2::Error::Io(error, "skipping a 7z entry".into())
                        })?;
                        Ok(true)
                    }
                    None => Ok(false),
                }
            })?;

            if let Some(error) = failure {
                return Err(error);
            }
        }
        Ok(())
    }
}

fn read_entry(file: &sevenz_rust2::ArchiveEntry, block: Option<BlockCoding>) -> Entry {
    let unix_mode = file
        .has_windows_attributes
        .then_some(file.windows_attributes)
        .and_then(mode::from_windows_attributes);

    let mut entry = Entry::new(
        path::normalize(&file.name),
        mode::kind_of(unix_mode, file.is_directory),
    );
    entry.size = file.size;
    entry.compressed_size = (file.compressed_size > 0).then_some(file.compressed_size);
    entry.modified = file
        .has_last_modified_date
        .then(|| file.last_modified_date.into());
    entry.mode = unix_mode;
    entry.crc32 = crc32_of(file);
    if let Some(block) = block {
        entry.encrypted = block.encrypted;
        entry.compression = Some(block.compression);
    }
    entry
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "7z widens a 32-bit CRC to u64"
)]
fn crc32_of(file: &sevenz_rust2::ArchiveEntry) -> Option<u32> {
    file.has_crc.then_some(file.crc as u32)
}

fn read_verified(
    index: usize,
    file: &sevenz_rust2::ArchiveEntry,
    reader: &mut dyn Read,
    decrypting: bool,
    each: &mut EntrySink<'_>,
) -> Result<()> {
    let Some(expected) = crc32_of(file) else {
        return each(index, reader);
    };

    let mut verified = CrcReader::new(reader, expected);
    let outcome = each(index, &mut verified);

    match verified.integrity() {
        Integrity::Unbroken => outcome,
        _ if decrypting => Err(Error::WrongPassword),
        Integrity::ChecksumMismatch => Err(Error::Damaged(format!(
            "entry \"{}\" does not match its recorded checksum",
            file.name
        ))),
        Integrity::ReadFailed => outcome,
    }
}
