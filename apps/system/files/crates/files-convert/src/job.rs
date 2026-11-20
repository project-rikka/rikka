use rikka_sync::Mutex;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use files_core::Location;
use files_vfs::{Failure, Operation, Outcome, Reporter};
use rikka_file_operations::TaskKind;

use crate::error::ConvertError;
use crate::landing::Landing;
use crate::model::convert_model;
use crate::movie::convert_movie;
use crate::picture;
use crate::progress::{Counter, Watch};
use crate::sound::{convert_sound, extract};
use crate::strip::strip;
use crate::target::{Target, extension_of};
use crate::tuning::Tuning;

const PICTURE_WORKERS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub sources: Vec<PathBuf>,
    pub target: Target,
    pub tuning: Tuning,
    pub suffix: String,
}

#[must_use]
pub fn convert(request: Request) -> Operation {
    rikka_file_operations::spawn(TaskKind::Convert, move |reporter| run(&request, reporter))
}

fn run(request: &Request, reporter: &Reporter) -> Outcome {
    let sizes: Vec<u64> = request
        .sources
        .iter()
        .map(|source| source.metadata().map_or(0, |metadata| metadata.len()))
        .collect();
    let counter = Counter::new(reporter, sizes.len() as u64, sizes.iter().sum());
    let failures = Mutex::new(Vec::new());
    let work = |index: usize| {
        if counter.is_cancelled() {
            return;
        }
        let source = &request.sources[index];
        match one(source, request, &Watch::new(&counter, sizes[index])) {
            Ok(()) => counter.finished(sizes[index]),
            Err(ConvertError::Cancelled) => {}
            Err(error) => {
                let at = Location::from_path(source);
                let failure = Failure::new(at.clone(), error.at(&at));
                failures.lock().push(failure);
                counter.finished(sizes[index]);
            }
        }
    };

    let workers = match request.target {
        Target::Picture(_) | Target::Shrink(_) | Target::StripMetadata => {
            std::thread::available_parallelism()
                .map_or(1, usize::from)
                .min(PICTURE_WORKERS)
                .min(request.sources.len())
        }
        _ => 1,
    };
    if workers > 1 {
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= request.sources.len() {
                            break;
                        }
                        work(index);
                    }
                });
            }
        });
    } else {
        (0..request.sources.len()).for_each(work);
    }

    Outcome {
        cancelled: reporter.is_cancelled(),
        failures: failures.into_inner(),
        ..Outcome::default()
    }
}

fn one(source: &Path, request: &Request, watch: &Watch) -> Result<(), ConvertError> {
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| ConvertError::Other(format!("{} names no file", source.display())))?;
    if request.target.matches_format(&name) {
        return Ok(());
    }
    let folder = source.parent().unwrap_or(Path::new("."));
    let extension = request
        .target
        .extension()
        .unwrap_or_else(|| extension_of(&name));
    let output = output_name(&name, extension, &request.suffix);
    let tuning = request.tuning;
    let write_output = |write: &dyn Fn(&Path) -> Result<(), ConvertError>| {
        let landing = Landing::file(folder, &output);
        write(landing.draft())?;
        if watch.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        landing.settle()?;
        Ok(())
    };
    match request.target {
        Target::Picture(format) => {
            write_output(&|draft| picture::convert(source, format, tuning, draft))
        }
        Target::Shrink(scale) => write_output(&|draft| picture::shrink(source, scale, draft)),
        Target::StripMetadata => write_output(&|draft| strip(source, draft)),
        Target::Sound(format) => {
            write_output(&|draft| convert_sound(source, format, tuning.quality, draft, watch))
        }
        Target::Extract(format) => {
            write_output(&|draft| extract(source, format, tuning.quality, draft, watch))
        }
        Target::Movie(format) => {
            write_output(&|draft| convert_movie(source, format, tuning, draft, watch))
        }
        Target::Model(format) => {
            convert_model(source, format, folder, &output, watch)?;
            Ok(())
        }
    }
}

fn output_name(name: &str, extension: &str, suffix: &str) -> String {
    let stem = rikka_format::name_stem(name);
    if extension_of(name).eq_ignore_ascii_case(extension) {
        format!("{stem}{suffix}.{extension}")
    } else {
        format!("{stem}.{extension}")
    }
}
