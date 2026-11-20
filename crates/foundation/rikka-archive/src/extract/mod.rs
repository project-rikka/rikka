mod fs;
mod options;
mod report;

pub use options::{ExtractOptions, ExtractSummary, Overwrite};
pub use report::{MetadataKind, MetadataRejection, MetadataReport};

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::entry::{Entry, EntryKind};
use crate::error::{Error, Result};
use crate::path;
use crate::progress::{Metered, Reporter};
use crate::reader::Archive;

pub(crate) fn run(
    archive: &mut Archive,
    destination: &Path,
    indices: &[usize],
    options: ExtractOptions<'_>,
) -> Result<ExtractSummary> {
    let mut options = options;
    let progress = options.progress.take();
    let cancel = options.cancel.take();
    let mut filter = options.filter.take();

    if let Some(password) = options.password.take() {
        archive.set_password(password);
    }

    let mut summary = ExtractSummary::default();
    let mut targets = Vec::with_capacity(indices.len());
    for &index in indices {
        let entry = archive.entry(index)?;

        let accepted = filter.as_mut().is_none_or(|filter| filter(entry));
        let stripped = strip(entry.name(), options.strip_components);
        match (accepted, stripped) {
            (true, Some(name)) => targets.push((index, path::relative(&name)?)),
            _ => summary.skipped += 1,
        }
    }

    let total_size = targets
        .iter()
        .map(|&(index, _)| archive.entries()[index].size)
        .sum();

    std::fs::create_dir_all(destination)?;

    let mut reporter = Reporter::new(progress, cancel);
    reporter.expect(targets.len(), total_size);

    let extraction = Extraction {
        archive,
        destination,
        options,
        reporter,
        summary,
        pending_directories: Vec::new(),
        last_directory: None,
    };
    extraction.run(&targets)
}

fn strip(name: &str, count: usize) -> Option<String> {
    if count == 0 {
        return Some(name.to_owned());
    }

    let mut components = name.trim_end_matches('/').split('/');
    for _ in 0..count {
        components.next()?;
    }

    let rest = components.collect::<Vec<_>>().join("/");
    let rest = if name.ends_with('/') && !rest.is_empty() {
        format!("{rest}/")
    } else {
        rest
    };
    (!rest.is_empty()).then_some(rest)
}

struct PendingFile {
    index: usize,
    position: usize,
    path: PathBuf,
    name: String,
    size: u64,
    mode: Option<u32>,
    modified: Option<SystemTime>,
}

impl PendingFile {
    fn new(index: usize, position: usize, path: PathBuf, entry: &Entry) -> PendingFile {
        PendingFile {
            index,
            position,
            path,
            name: entry.name.clone(),
            size: entry.size,
            mode: entry.mode,
            modified: entry.modified,
        }
    }
}

struct Extraction<'a, 'b> {
    archive: &'a mut Archive,
    destination: &'a Path,
    options: ExtractOptions<'b>,
    reporter: Reporter<'b>,
    summary: ExtractSummary,
    pending_directories: Vec<(PathBuf, SystemTime)>,
    last_directory: Option<PathBuf>,
}

impl Extraction<'_, '_> {
    fn run(mut self, targets: &[(usize, PathBuf)]) -> Result<ExtractSummary> {
        let mut files = Vec::new();
        let mut links = Vec::new();

        for (position, (index, relative)) in targets.iter().enumerate() {
            let entry = self.archive.entry(*index)?;
            let kind = entry.kind;
            let path = self.destination.join(relative);

            match kind {
                EntryKind::File => files.push(PendingFile::new(*index, position, path, entry)),
                EntryKind::Symlink | EntryKind::HardLink => links.push((*index, position, path)),
                EntryKind::Directory => {
                    let entry = entry.clone();
                    self.reporter.report(&entry.name, position, 0, 0)?;
                    self.write_directory(&entry, &path)?;
                }
                EntryKind::Special => self.summary.skipped += 1,
            }
        }

        self.write_files(files)?;

        for (index, position, path) in links {
            let entry = self.archive.entry(index)?.clone();
            self.reporter.report(&entry.name, position, 0, 0)?;
            if entry.kind == EntryKind::Symlink {
                self.write_symlink(&entry, index, &path)?;
            } else {
                self.write_hard_link(&entry, &path)?;
            }
        }

        self.finish_directories();
        self.summary.bytes = self.reporter.total_bytes();
        Ok(self.summary)
    }

    fn write_files(&mut self, files: Vec<PendingFile>) -> Result<()> {
        let mut pending = Vec::with_capacity(files.len());
        for file in files {
            if self.prepare_target(&file.path)? {
                pending.push(file);
            }
        }
        if pending.is_empty() {
            return Ok(());
        }

        let indices: Vec<usize> = pending.iter().map(|file| file.index).collect();
        let permissions = self.options.permissions;
        let timestamps = self.options.timestamps;

        let Extraction {
            archive,
            reporter,
            summary,
            ..
        } = &mut *self;
        let metadata = &mut summary.metadata;
        let mut cursor = 0;

        let result = archive.read_entries(&indices, &mut |_, reader| {
            let file = &pending[cursor];
            cursor += 1;

            reporter.report(&file.name, file.position, 0, file.size)?;

            let sink = BufWriter::new(File::create(&file.path)?);
            let mut sink = Metered::new(sink, reporter, &file.name, file.position, file.size);
            std::io::copy(reader, &mut sink)?;
            sink.flush()?;

            if permissions && let Some(mode) = file.mode {
                metadata.record(
                    &file.path,
                    MetadataKind::Permissions,
                    fs::apply_mode(&file.path, mode),
                );
            }
            if timestamps && let Some(modified) = file.modified {
                metadata.record(
                    &file.path,
                    MetadataKind::Timestamp,
                    fs::apply_modified(&file.path, modified),
                );
            }
            Ok(())
        });

        if let Err(error) = result {
            return Err(self.reporter.unwrap_cancellation(error));
        }

        self.summary.files += pending.len();
        Ok(())
    }

    fn write_directory(&mut self, entry: &Entry, path: &Path) -> Result<()> {
        self.ensure_directory(path)?;
        if self.options.permissions
            && let Some(mode) = entry.mode
        {
            self.summary.metadata.record(
                path,
                MetadataKind::Permissions,
                fs::apply_mode(path, mode),
            );
        }
        if self.options.timestamps
            && let Some(modified) = entry.modified
        {
            self.pending_directories.push((path.to_owned(), modified));
        }
        self.summary.directories += 1;
        Ok(())
    }

    fn write_symlink(&mut self, entry: &Entry, index: usize, path: &Path) -> Result<()> {
        if !self.options.symlinks {
            self.summary.skipped += 1;
            return Ok(());
        }
        if !self.prepare_target(path)? {
            return Ok(());
        }

        let target = self.archive.read_link(index)?;
        if !path::link_stays_within(self.destination, path, &target) {
            return Err(Error::UnsafeEntryPath(format!(
                "{} -> {target}",
                entry.name
            )));
        }

        if let Err(error) = fs::create_symlink(&target, path) {
            tracing::warn!(link = %entry.name, %error, "failed to create symlink");
            self.summary.skipped += 1;
            return Ok(());
        }

        self.summary.links += 1;
        Ok(())
    }

    fn write_hard_link(&mut self, entry: &Entry, path: &Path) -> Result<()> {
        let Some(target) = entry.link_target() else {
            self.summary.skipped += 1;
            return Ok(());
        };
        let target_path = path::join(self.destination, target)?;
        if let Some(folder) = target_path.parent() {
            self.stay_on_real_folders(folder)?;
        }

        if !self.prepare_target(path)? {
            return Ok(());
        }

        if let Err(error) = std::fs::hard_link(&target_path, path) {
            tracing::warn!(link = %entry.name, %error, "failed to create hard link");
            self.summary.skipped += 1;
            return Ok(());
        }

        self.summary.links += 1;
        Ok(())
    }

    fn prepare_target(&mut self, path: &Path) -> Result<bool> {
        if let Some(parent) = path.parent() {
            self.ensure_directory(parent)?;
        }
        if fs::prepare_destination(path, self.options.overwrite)? {
            return Ok(true);
        }
        self.summary.skipped += 1;
        Ok(false)
    }

    fn ensure_directory(&mut self, path: &Path) -> Result<()> {
        if self.last_directory.as_deref() == Some(path) {
            return Ok(());
        }
        self.stay_on_real_folders(path)?;
        std::fs::create_dir_all(path)?;
        self.last_directory = Some(path.to_owned());
        Ok(())
    }

    fn stay_on_real_folders(&self, path: &Path) -> Result<()> {
        if path::crosses_link(self.destination, path)? {
            return Err(Error::UnsafeEntryPath(path.display().to_string()));
        }
        Ok(())
    }

    fn finish_directories(&mut self) {
        let Extraction {
            pending_directories,
            summary,
            ..
        } = self;
        for (path, modified) in pending_directories.iter().rev() {
            summary.metadata.record(
                path,
                MetadataKind::Timestamp,
                fs::apply_modified(path, *modified),
            );
        }
    }
}
