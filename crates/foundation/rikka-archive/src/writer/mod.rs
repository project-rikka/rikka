mod options;
mod sevenz;
mod single;
mod tar;
mod walk;
mod zip;

pub use options::CreateOptions;

use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::Path;
use std::time::SystemTime;

use crate::error::{Error, Result};
use crate::format::Format;
use crate::path;
use crate::progress::{Metered, Reporter};

use walk::SourceKind;

pub(crate) struct NewEntry {
    pub(crate) name: String,
    pub(crate) mode: Option<u32>,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) size: u64,
}

impl NewEntry {
    fn file(name: &str, size: u64) -> Result<NewEntry> {
        Ok(NewEntry {
            name: path::validate(name)?,
            mode: None,
            modified: Some(SystemTime::now()),
            size,
        })
    }

    fn directory(name: &str) -> Result<NewEntry> {
        let mut entry = NewEntry::file(name, 0)?;
        entry.name = format!("{}/", entry.name.trim_end_matches('/'));
        Ok(entry)
    }

    fn from_source(source: walk::Source) -> Result<NewEntry> {
        Ok(NewEntry {
            size: source.size(),
            name: path::validate(&source.name)?,
            mode: source.mode,
            modified: source.modified,
        })
    }
}

pub(crate) trait WriterBackend {
    fn add_file(&mut self, entry: &NewEntry, data: &mut dyn Read) -> Result<()>;

    fn add_directory(&mut self, _entry: &NewEntry) -> Result<()> {
        Err(Error::Unsupported(
            "this format holds a single file".to_owned(),
        ))
    }

    fn add_symlink(&mut self, _entry: &NewEntry, _target: &str) -> Result<()> {
        Err(Error::Unsupported(
            "this format holds a single file".to_owned(),
        ))
    }

    fn finish(self: Box<Self>) -> Result<()>;
}

pub struct ArchiveWriter<'a> {
    backend: Box<dyn WriterBackend>,
    format: Format,
    follow_symlinks: bool,
    reporter: Reporter<'a>,
    index: usize,
}

impl<'a> ArchiveWriter<'a> {
    pub fn create(path: impl AsRef<Path>) -> Result<ArchiveWriter<'a>> {
        ArchiveWriter::create_with(path, CreateOptions::new())
    }

    pub fn create_with(
        path: impl AsRef<Path>,
        options: CreateOptions<'a>,
    ) -> Result<ArchiveWriter<'a>> {
        let path = path.as_ref();
        let format = Format::from_path(path).ok_or(Error::UnknownFormat)?;
        ArchiveWriter::create_as(path, format, options)
    }

    pub fn create_as(
        path: impl AsRef<Path>,
        format: Format,
        options: CreateOptions<'a>,
    ) -> Result<ArchiveWriter<'a>> {
        let file = BufWriter::new(File::create(path)?);
        ArchiveWriter::new(file, format, options)
    }

    pub fn new<W: Write + Seek + 'static>(
        sink: W,
        format: Format,
        options: CreateOptions<'a>,
    ) -> Result<ArchiveWriter<'a>> {
        let mut options = options;
        let progress = options.progress.take();
        let cancel = options.cancel.take();
        let follow_symlinks = options.follow_symlinks;

        if options.password.is_some() && !format.supports_encryption() {
            return Err(Error::Unsupported(format!(
                "{format} has no encryption; use zip or 7z"
            )));
        }

        let backend: Box<dyn WriterBackend> = match format {
            Format::Zip => Box::new(zip::ZipWriterBackend::new(sink, &options)),
            Format::SevenZ => Box::new(sevenz::SevenZWriterBackend::new(sink, &options)?),
            Format::Tar | Format::TarGzip | Format::TarBzip2 | Format::TarXz | Format::TarZstd => {
                Box::new(tar::TarWriterBackend::new(
                    sink,
                    format.stream_compression(),
                    &options,
                )?)
            }
            Format::Gzip | Format::Bzip2 | Format::Xz | Format::Zstd => {
                let Some(compression) = format.stream_compression() else {
                    return Err(Error::UnknownFormat);
                };
                Box::new(single::SingleWriterBackend::new(
                    sink,
                    compression,
                    &options,
                ))
            }
        };

        Ok(ArchiveWriter {
            backend,
            format,
            follow_symlinks,
            reporter: Reporter::new(progress, cancel),
            index: 0,
        })
    }

    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    pub fn add_bytes(&mut self, name: &str, data: &[u8]) -> Result<()> {
        let entry = NewEntry::file(name, data.len() as u64)?;
        self.reporter.expect(1, entry.size);
        self.write_file(entry, &mut &data[..])
    }

    pub fn add_reader(&mut self, name: &str, size: u64, data: impl Read) -> Result<()> {
        let entry = NewEntry::file(name, size)?;
        self.reporter.expect(1, size);
        let mut data = data;
        self.write_file(entry, &mut data)
    }

    pub fn add_directory(&mut self, name: &str) -> Result<()> {
        let entry = NewEntry::directory(name)?;
        self.reporter.expect(1, 0);
        self.write_directory(entry)
    }

    pub fn add_symlink(&mut self, name: &str, target: &str) -> Result<()> {
        let mut entry = NewEntry::file(name, 0)?;
        entry.mode = Some(0o777);
        self.reporter.expect(1, 0);
        self.write_symlink(entry, target)
    }

    pub fn add_path(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| Error::Unsupported(format!("path {} has no file name", path.display())))?
            .to_owned();
        self.add_path_as(path, &name)
    }

    pub fn add_path_as(&mut self, path: impl AsRef<Path>, name: &str) -> Result<()> {
        let sources = walk::collect(path.as_ref(), &path::validate(name)?, self.follow_symlinks)?;
        let bytes = sources.iter().map(walk::Source::size).sum();
        self.reporter.expect(sources.len(), bytes);

        for source in sources {
            let path = source.path.clone();
            let kind = source.kind.clone();
            let entry = NewEntry::from_source(source)?;

            match kind {
                SourceKind::Directory => self.write_directory(entry)?,
                SourceKind::Symlink { target } => self.write_symlink(entry, &target)?,
                SourceKind::File { .. } => {
                    let mut file = BufReader::new(File::open(&path)?);
                    self.write_file(entry, &mut file)?;
                }
            }
        }
        Ok(())
    }

    pub fn finish(self) -> Result<()> {
        self.backend.finish()
    }

    fn write_file(&mut self, entry: NewEntry, data: &mut dyn Read) -> Result<()> {
        if !self.format.is_container() && self.index > 0 {
            return Err(Error::NotAContainer(self.format));
        }

        let index = self.index;
        self.reporter.report(&entry.name, index, 0, entry.size)?;

        let mut counted = Metered::new(data, &mut self.reporter, &entry.name, index, entry.size);
        let result = self.backend.add_file(&entry, &mut counted);

        result.map_err(|error| self.reporter.unwrap_cancellation(error))?;
        self.index += 1;
        Ok(())
    }

    fn write_directory(&mut self, entry: NewEntry) -> Result<()> {
        self.require_container()?;
        self.reporter.report(&entry.name, self.index, 0, 0)?;
        self.backend.add_directory(&entry)?;
        self.index += 1;
        Ok(())
    }

    fn write_symlink(&mut self, entry: NewEntry, target: &str) -> Result<()> {
        self.require_container()?;
        self.reporter.report(&entry.name, self.index, 0, 0)?;
        self.backend.add_symlink(&entry, target)?;
        self.index += 1;
        Ok(())
    }

    fn require_container(&self) -> Result<()> {
        if self.format.is_container() {
            Ok(())
        } else {
            Err(Error::NotAContainer(self.format))
        }
    }
}

impl fmt::Debug for ArchiveWriter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchiveWriter")
            .field("format", &self.format)
            .field("entries", &self.index)
            .finish_non_exhaustive()
    }
}
