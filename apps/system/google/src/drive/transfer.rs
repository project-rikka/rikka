use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use futures_channel::mpsc::{UnboundedSender, unbounded};
use google_drive::{Drive, Error, Export, File, Shelf};
use rikka_app::prelude::*;
use rikka_dialog::ChooserResponse;
use rikka_net::Progress;

use crate::drive::{DriveState, Notice};

pub(super) const RUNNING: &str = "google-drive-transfer";
pub(super) const PROGRESS: &str = "google-drive-transfer-progress";
pub(super) const OPENED_BUDGET: u64 = 2 * 1024 * 1024 * 1024;

const PERCENT: f32 = 100.0;

#[derive(Clone, Debug)]
pub(crate) enum Job {
    Open(Box<File>),
    Download {
        files: Vec<File>,
        folder: PathBuf,
        pdf: bool,
    },
    Upload {
        paths: Vec<PathBuf>,
        parent: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct Transfer {
    pub(crate) label: String,
    pub(crate) fraction: Option<f32>,
    pub(crate) cancel: CancelToken,
}

#[derive(Debug)]
pub(super) enum Report {
    Began(String),
    Moved(f32),
}

#[derive(Debug)]
pub(super) enum Outcome {
    Opened(PathBuf),
    Downloaded(PathBuf, usize),
    Uploaded,
}

impl PartialEq for Transfer {
    fn eq(&self, other: &Transfer) -> bool {
        self.label == other.label && self.fraction == other.fraction
    }
}

impl DriveState {
    pub(crate) fn enqueue(&mut self, job: Job) {
        self.queue.push_back(job);
        if self.transfer.is_none() {
            self.run_next();
        }
    }

    pub(crate) fn cancel_transfer(&mut self) {
        self.queue.clear();
        if let Some(transfer) = self.transfer.as_ref() {
            transfer.cancel.cancel();
        }
    }

    pub(crate) fn reveal_download(&mut self, folder: &Path) {
        self.notice.set(None);
        if let Err(error) = rikka_open::reveal(folder) {
            self.notify(error.to_string());
        }
    }

    fn run_next(&mut self) {
        let (Some(session), Some(job)) = (&self.session, self.queue.pop_front()) else {
            self.transfer.set(None);
            return;
        };
        let cancel = CancelToken::new();
        let (reports, reported) = unbounded();
        self.transfer.set(Some(Transfer {
            label: String::new(),
            fraction: None,
            cancel: cancel.clone(),
        }));
        stream_keyed(
            PROGRESS,
            reported,
            |drive: &mut DriveState, report: Report| {
                let Some(transfer) = drive.transfer.as_mut() else {
                    return skip_view();
                };
                match report {
                    Report::Began(label) => {
                        transfer.label = label;
                        transfer.fraction = None;
                    }
                    Report::Moved(fraction) => transfer.fraction = Some(fraction),
                }
            },
        );
        let (drive, shelf) = (session.drive.clone(), session.shelf.clone());
        spawn_keyed(
            RUNNING,
            async move { run(&drive, &shelf, job, cancel, reports).await },
            |drive: &mut DriveState, outcome: google_drive::Result<Outcome>| {
                drive.transferred(outcome);
            },
        );
    }

    fn transferred(&mut self, outcome: google_drive::Result<Outcome>) {
        cancel(PROGRESS);
        match outcome {
            Ok(Outcome::Opened(path)) => {
                if let Err(error) = rikka_open::open(&path.to_string_lossy()) {
                    self.notify(error.to_string());
                }
            }
            Ok(Outcome::Downloaded(folder, count)) => {
                self.notice.set(Some(Notice {
                    text: tr!("google.drive.downloaded", count = count).to_string(),
                    restores: Vec::new(),
                    danger: false,
                    folder: Some(folder),
                }));
            }
            Ok(Outcome::Uploaded) => self.refresh(),
            Err(error) if error.is_cancelled() => {}
            Err(error) => self.failed(&error),
        }
        self.run_next();
    }
}

pub(super) fn chosen_paths(chosen: rikka_dialog::Result<ChooserResponse>) -> Vec<PathBuf> {
    match chosen {
        Ok(ChooserResponse::Files(paths)) => paths,
        Ok(ChooserResponse::Cancelled) => Vec::new(),
        Err(error) => {
            tracing::warn!("failed to ask for files: {error}");
            Vec::new()
        }
    }
}

async fn run(
    drive: &Drive,
    shelf: &Shelf,
    job: Job,
    cancel: CancelToken,
    reports: UnboundedSender<Report>,
) -> google_drive::Result<Outcome> {
    let began = |label: String| {
        let _ = reports.unbounded_send(Report::Began(label));
    };
    match job {
        Job::Open(file) => {
            let destination = shelf.opened(&file, None);
            if !destination.is_file() {
                began(tr!("google.drive.opening", name = file.name.as_str()).to_string());
                prepare(&destination)?;
                drive
                    .download(&file, &destination, cancel, meter(&reports))
                    .await?;
            }
            Ok(Outcome::Opened(destination))
        }
        Job::Download { files, folder, pdf } => {
            let count = files.len();
            for file in files {
                began(tr!("google.drive.downloading", name = file.name.as_str()).to_string());
                let export = match (file.is_native(), pdf) {
                    (false, _) => None,
                    (true, true) => Some(Export::PDF),
                    (true, false) => Some(Export::editable(file.kind()).unwrap_or(Export::PDF)),
                };
                let name =
                    rikka_format::safe_file_name(&file.name).unwrap_or_else(|| file.id.clone());
                let name = match export {
                    Some(export) => format!("{name}.{}", export.extension),
                    None => name,
                };
                let name = rikka_format::unique_name(&name, |taken| folder.join(taken).exists());
                let destination = folder.join(name);
                match export {
                    Some(export) => {
                        drive
                            .export(&file, export, &destination, cancel.clone())
                            .await?;
                    }
                    None => {
                        drive
                            .download(&file, &destination, cancel.clone(), meter(&reports))
                            .await?;
                    }
                }
            }
            Ok(Outcome::Downloaded(folder, count))
        }
        Job::Upload { paths, parent } => {
            for path in paths {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                began(tr!("google.drive.uploading", name = name.as_str()).to_string());
                drive
                    .upload(&path, &parent, &name, cancel.clone(), meter(&reports))
                    .await?;
            }
            Ok(Outcome::Uploaded)
        }
    }
}

fn prepare(destination: &Path) -> google_drive::Result<()> {
    let Some(folder) = destination.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(folder).map_err(|source| Error::File {
        path: folder.to_owned(),
        source,
    })
}

fn meter(reports: &UnboundedSender<Report>) -> impl Fn(Progress) + Clone + Send + 'static {
    let reports = reports.clone();
    let last = Arc::new(AtomicU32::new(u32::MAX));
    move |progress: Progress| {
        let Some(fraction) = progress.fraction() else {
            return;
        };
        let percent = (fraction * PERCENT) as u32;
        if last.swap(percent, Ordering::AcqRel) != percent {
            let _ = reports.unbounded_send(Report::Moved(fraction));
        }
    }
}
