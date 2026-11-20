mod sevenz;
mod single;
mod tar;
mod verify;
mod zip;

use std::fmt;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, Write};
use std::path::Path;

use crate::detect;
use crate::encoding;
use crate::entry::{Entry, EntryKind};
use crate::error::{Error, Result};
use crate::extract::{ExtractOptions, ExtractSummary};
use crate::format::Format;
use crate::password::Password;

pub type EntrySink<'a> = dyn FnMut(usize, &mut dyn Read) -> Result<()> + 'a;

pub(crate) trait ReaderBackend {
    fn entries(&self) -> &[Entry];

    fn read_entries(
        &mut self,
        indices: &[usize],
        password: &Password,
        each: &mut EntrySink<'_>,
    ) -> Result<()>;

    fn comment(&self) -> Option<String> {
        None
    }
}

pub struct Archive {
    format: Format,
    backend: Box<dyn ReaderBackend>,
    password: Password,
}

impl fmt::Debug for Archive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Archive")
            .field("format", &self.format)
            .field("entries", &self.len())
            .finish_non_exhaustive()
    }
}

impl Archive {
    pub fn open(path: impl AsRef<Path>) -> Result<Archive> {
        Archive::open_with_password(path, Password::default())
    }

    pub fn open_with_password(
        path: impl AsRef<Path>,
        password: impl Into<Password>,
    ) -> Result<Archive> {
        let path = path.as_ref();
        let mut source = BufReader::new(File::open(path)?);

        let format = match detect::detect_reader(&mut source) {
            Ok(format) => format,
            Err(error) => Format::from_path(path).ok_or(error)?,
        };
        let name_hint = path.file_name().and_then(|name| name.to_str());

        Archive::with_source(format, source, password.into(), name_hint)
    }

    pub fn from_reader<R: Read + Seek + 'static>(source: R) -> Result<Archive> {
        Archive::from_reader_with_password(source, Password::default())
    }

    pub fn from_reader_with_password<R: Read + Seek + 'static>(
        mut source: R,
        password: impl Into<Password>,
    ) -> Result<Archive> {
        let format = detect::detect_reader(&mut source)?;
        Archive::with_source(format, source, password.into(), None)
    }

    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Archive> {
        Archive::from_reader(Cursor::new(bytes.into()))
    }

    fn with_source<R: Read + Seek + 'static>(
        format: Format,
        source: R,
        password: Password,
        name_hint: Option<&str>,
    ) -> Result<Archive> {
        let backend: Box<dyn ReaderBackend> = match format {
            Format::Zip => Box::new(zip::ZipReader::new(source)?),
            Format::SevenZ => Box::new(sevenz::SevenZReader::new(source, &password)?),
            Format::Tar | Format::TarGzip | Format::TarBzip2 | Format::TarXz | Format::TarZstd => {
                Box::new(tar::TarReader::new(source, format.stream_compression())?)
            }
            Format::Gzip | Format::Bzip2 | Format::Xz | Format::Zstd => {
                let Some(compression) = format.stream_compression() else {
                    return Err(Error::UnknownFormat);
                };
                let name = name_hint.map(|hint| format.strip_extension(hint));
                Box::new(single::SingleReader::new(source, compression, name)?)
            }
        };

        Ok(Archive {
            format,
            backend,
            password,
        })
    }

    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        self.backend.entries()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries().is_empty()
    }

    pub fn entry(&self, index: usize) -> Result<&Entry> {
        self.entries().get(index).ok_or(Error::IndexOutOfRange {
            index,
            count: self.len(),
        })
    }

    #[must_use]
    pub fn find(&self, name: &str) -> Option<usize> {
        let target = crate::path::normalize(name);
        let target = target.trim_end_matches('/');
        self.entries()
            .iter()
            .position(|entry| entry.name.trim_end_matches('/') == target)
    }

    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.entries().iter().any(Entry::is_encrypted)
    }

    #[must_use]
    pub fn comment(&self) -> Option<String> {
        self.backend.comment()
    }

    pub fn set_password(&mut self, password: impl Into<Password>) {
        self.password = password.into();
    }

    pub fn read_entry(&mut self, index: usize) -> Result<Vec<u8>> {
        let size = usize::try_from(self.entry(index)?.size).unwrap_or(0);
        let mut data = Vec::with_capacity(size);
        self.copy_entry(index, &mut data)?;
        Ok(data)
    }

    pub fn read_entry_by_name(&mut self, name: &str) -> Result<Vec<u8>> {
        let index = self
            .find(name)
            .ok_or_else(|| Error::EntryNotFound(name.to_owned()))?;
        self.read_entry(index)
    }

    pub fn copy_entry(&mut self, index: usize, sink: &mut dyn Write) -> Result<u64> {
        self.entry(index)?;

        let mut written = 0;
        self.read_entries(&[index], &mut |_, reader| {
            written = std::io::copy(reader, sink)?;
            Ok(())
        })?;
        Ok(written)
    }

    pub fn read_each(&mut self, indices: &[usize], take: &mut EntrySink<'_>) -> Result<()> {
        let mut sorted: Vec<usize> = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        for &index in &sorted {
            self.entry(index)?;
        }
        self.read_entries(&sorted, take)
    }

    pub(crate) fn read_entries(
        &mut self,
        indices: &[usize],
        each: &mut EntrySink<'_>,
    ) -> Result<()> {
        self.backend.read_entries(indices, &self.password, each)
    }

    pub fn read_link(&mut self, index: usize) -> Result<String> {
        let entry = self.entry(index)?;
        if let Some(target) = &entry.link_target {
            return Ok(target.clone());
        }
        if !matches!(entry.kind, EntryKind::Symlink | EntryKind::HardLink) {
            return Err(Error::Unsupported(format!(
                "entry \"{}\" is not a link",
                entry.name
            )));
        }

        let data = self.read_entry(index)?;
        Ok(encoding::decode_name(&data))
    }

    pub fn extract(&mut self, destination: impl AsRef<Path>) -> Result<ExtractSummary> {
        self.extract_with(destination, ExtractOptions::new())
    }

    pub fn extract_with(
        &mut self,
        destination: impl AsRef<Path>,
        options: ExtractOptions<'_>,
    ) -> Result<ExtractSummary> {
        let indices = (0..self.len()).collect::<Vec<_>>();
        crate::extract::run(self, destination.as_ref(), &indices, options)
    }

    pub fn extract_entries(
        &mut self,
        destination: impl AsRef<Path>,
        indices: &[usize],
        options: ExtractOptions<'_>,
    ) -> Result<ExtractSummary> {
        crate::extract::run(self, destination.as_ref(), indices, options)
    }
}
