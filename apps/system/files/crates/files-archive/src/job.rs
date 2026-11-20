use std::collections::HashSet;
use std::path::{Path, PathBuf};

use files_core::Location;
use files_vfs::{Failure, Operation, Outcome, Reporter};
use rikka_archive::{
    Archive, ArchiveWriter, CreateOptions, EntryKind, ExtractOptions, Format, Level, Progress,
};
use rikka_file_operations::{Plan, TaskKind};
use rikka_format::unique_name;
use rikka_types::Flow;

use crate::error::ArchiveError;
use crate::format;

#[derive(Clone, Debug)]
pub struct Packing {
    pub sources: Vec<PathBuf>,
    pub into: PathBuf,
    pub format: Format,
    pub level: Level,
    pub password: Option<String>,
}

#[must_use]
pub fn compress(packing: Packing) -> Operation {
    rikka_file_operations::spawn(TaskKind::Compress, move |reporter| {
        let at = Location::from_path(&packing.into);
        let planned = plan(&packing.sources, reporter);
        reporter.totals(planned.items, planned.bytes);

        let packed = pack(&packing, reporter);
        if packed.is_err() {
            std::fs::remove_file(&packing.into).ok();
        }
        outcome(&at, packed)
    })
}

#[must_use]
pub fn extract(archive: PathBuf, beside: PathBuf, password: Option<String>) -> Operation {
    rikka_file_operations::spawn(TaskKind::Extract, move |reporter| {
        let at = Location::from_path(&archive);
        let unpacked = unpack(&archive, &beside, password.as_deref(), reporter);
        outcome(&at, unpacked)
    })
}

fn destination(archive: &Path, beside: &Path) -> PathBuf {
    let name = format::file_name(archive);
    let stem = Format::split(&name).map_or(name.as_str(), |(stem, _)| stem);
    let folder = unique_name(stem, |candidate| {
        beside.join(candidate).symlink_metadata().is_ok()
    });
    beside.join(folder)
}

fn pack(packing: &Packing, reporter: &Reporter) -> Result<(), ArchiveError> {
    if packing.into.symlink_metadata().is_ok() {
        return Err(ArchiveError::Exists);
    }
    let mut options = CreateOptions::new()
        .level(packing.level)
        .on_progress(watching(reporter.clone()));
    if let Some(password) = &packing.password {
        options = options.password(password.as_str());
    }

    let mut writer = ArchiveWriter::create_as(&packing.into, packing.format, options)
        .map_err(|error| ArchiveError::from_archive(&error))?;
    let mut used_names: HashSet<String> = HashSet::with_capacity(packing.sources.len());
    for source in &packing.sources {
        let name = unique_name(&format::file_name(source), |candidate| {
            used_names.contains(candidate)
        });
        writer
            .add_path_as(source, &name)
            .map_err(|error| ArchiveError::from_archive(&error))?;
        used_names.insert(name);
    }
    writer
        .finish()
        .map_err(|error| ArchiveError::from_archive(&error))
}

fn unpack(
    archive: &Path,
    beside: &Path,
    password: Option<&str>,
    reporter: &Reporter,
) -> Result<(), ArchiveError> {
    let mut opened = match password {
        Some(password) => Archive::open_with_password(archive, password),
        None => Archive::open(archive),
    }
    .map_err(|error| ArchiveError::from_archive(&error))?;

    let into = if wrapped(&opened) {
        beside.to_path_buf()
    } else {
        destination(archive, beside)
    };
    std::fs::create_dir_all(&into).map_err(|error| ArchiveError::Other(error.to_string()))?;

    let options = ExtractOptions::new().on_progress(unpacking(reporter.clone()));
    let summary = opened
        .extract_with(&into, options)
        .map_err(|error| ArchiveError::from_archive(&error))?;

    summary.metadata.warn();
    Ok(())
}

fn wrapped(archive: &Archive) -> bool {
    let mut root: Option<String> = None;
    for entry in archive.entries() {
        let Some(first) = entry.name().split('/').find(|part| !part.is_empty()) else {
            continue;
        };
        if !entry.name().trim_start_matches('/').contains('/')
            && entry.kind() != EntryKind::Directory
        {
            return false;
        }
        match &root {
            Some(root) if root != first => return false,
            Some(_) => {}
            None => root = Some(first.to_owned()),
        }
    }
    root.is_some()
}

fn watching(reporter: Reporter) -> impl FnMut(Progress<'_>) -> Flow {
    move |progress| {
        reporter.reached(progress.entry_index as u64, progress.total_bytes);
        flow(&reporter)
    }
}

fn unpacking(reporter: Reporter) -> impl FnMut(Progress<'_>) -> Flow {
    let mut measured = false;
    move |progress| {
        if !std::mem::replace(&mut measured, true) {
            reporter.totals(progress.entry_count as u64, progress.total_size);
        }
        reporter.reached(progress.entry_index as u64, progress.total_bytes);
        flow(&reporter)
    }
}

fn flow(reporter: &Reporter) -> Flow {
    if reporter.is_cancelled() {
        Flow::Cancel
    } else {
        Flow::Continue
    }
}

fn outcome(at: &Location, result: Result<(), ArchiveError>) -> Outcome {
    match result {
        Ok(()) => Outcome::default(),
        Err(ArchiveError::Cancelled) => Outcome {
            cancelled: true,
            ..Outcome::default()
        },
        Err(error) => Outcome {
            failures: vec![Failure::new(at.clone(), error.at(at))],
            ..Outcome::default()
        },
    }
}

fn plan(sources: &[PathBuf], reporter: &Reporter) -> Plan {
    let mut plan = Plan::default();
    for source in sources {
        plan.add(measure(source, reporter));
    }
    plan
}

fn measure(at: &Path, reporter: &Reporter) -> Plan {
    let Ok(metadata) = at.symlink_metadata() else {
        return Plan::default();
    };
    if !metadata.is_dir() {
        return Plan {
            items: 1,
            bytes: metadata.len(),
        };
    }
    let Ok(reading) = std::fs::read_dir(at) else {
        return Plan { items: 1, bytes: 0 };
    };
    let mut plan = Plan { items: 1, bytes: 0 };
    for child in reading.flatten() {
        if reporter.is_cancelled() {
            break;
        }
        plan.add(measure(&child.path(), reporter));
    }
    plan
}
