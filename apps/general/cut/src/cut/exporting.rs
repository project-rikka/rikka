use std::path::{Path, PathBuf};
use std::time::Duration;

use cut_export::{Job, Phase, Range, Report, Settings, Target};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_dirs::{UserDir, UserDirs};
use rikka_image_edit::Imaging;
use rikka_ui::ComputedImage;

use super::Cut;
use super::sheet::{Sheet, draft, draft_mut};
use crate::export::{Exporting, Opening};

impl Cut {
    pub(crate) fn exporting(&self) -> Option<&Exporting> {
        draft(&self.sheet)
    }

    pub(crate) fn job(&self) -> Option<&Job> {
        self.job.as_ref()
    }

    pub(crate) fn export_preview(&self) -> Option<ImageSource> {
        self.export_preview.size()?;
        Some(ImageSource::Computed(self.export_preview.clone()))
    }

    pub(crate) fn is_modal(&self) -> bool {
        self.is_asking() || self.sheet.is_some() || self.job.is_some()
    }

    pub(crate) fn can_export(&self) -> bool {
        !self.project().is_empty() && !self.is_modal()
    }

    pub(crate) fn open_export(&mut self, target: Option<Target>) {
        if !self.can_export() {
            return;
        }
        self.commit_marker();
        self.commit_text();
        self.pause();
        let mut settings = self.preferences.export();
        if let Some(target) = target {
            settings.target = target;
        }
        self.export_sheets += 1;
        let project = self.project();
        self.sheet = Some(Sheet::Export(Box::new(Exporting::new(Opening {
            settings,
            range: Range::suggested(project),
            folder: self.export_folder(),
            stem: self.name(),
            frame: self.transport.frame(),
            key: self.export_sheets,
        }))));
    }

    fn export_folder(&self) -> PathBuf {
        let projects = self
            .document
            .path()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        let dirs = UserDirs::load();
        self.preferences
            .export_folder()
            .into_iter()
            .chain(projects)
            .chain(dirs.path(UserDir::Videos))
            .find(|folder| folder.is_dir())
            .or_else(|| dirs.home().map(Path::to_path_buf))
            .unwrap_or_else(std::env::temp_dir)
    }

    pub(crate) fn tune_export(&mut self, change: impl FnOnce(&mut Settings)) {
        if let Some(exporting) = draft_mut::<Exporting>(&mut self.sheet) {
            exporting.tune(change);
        }
    }

    pub(crate) fn select_export_range(&mut self, range: Range) {
        if let Some(exporting) = draft_mut::<Exporting>(&mut self.sheet) {
            exporting.select_range(range);
        }
    }

    pub(crate) fn toggle_export_details(&mut self) {
        if let Some(exporting) = draft_mut::<Exporting>(&mut self.sheet) {
            exporting.toggle_details();
        }
    }

    pub(crate) fn close_export_sheet(&mut self) {
        self.sheet = None;
    }

    pub(crate) fn select_export_location(&mut self) {
        let Some(exporting) = draft_mut::<Exporting>(&mut self.sheet) else {
            return;
        };
        if exporting.is_asking() {
            return;
        }
        exporting.ask(true);
        let settings = exporting.settings();
        let named = exporting
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let chooser = Chooser::save(tr!("cut.export.choose").to_string())
            .parent(window_identifier())
            .accept_label(tr!("cut.export.accept").to_string())
            .current_name(named)
            .filter(
                Filter::named(settings.extension().to_uppercase()).mime_type(settings.mime_type()),
            )
            .current_folder(exporting.folder());
        spawn_blocking(
            move || chooser.run(),
            |cut: &mut Cut, selected: rikka_dialog::Result<ChooserResponse>| {
                let Some(exporting) = draft_mut::<Exporting>(&mut cut.sheet) else {
                    return;
                };
                exporting.ask(false);
                match selected {
                    Ok(ChooserResponse::Files(paths)) => {
                        if let Some(path) = paths.into_iter().next() {
                            exporting.selected(path);
                        }
                    }
                    Ok(ChooserResponse::Cancelled) => {}
                    Err(trouble) => {
                        exporting.fail(tr!("cut.export.unasked", reason = &trouble.to_string()))
                    }
                }
            },
        );
    }

    pub(crate) fn export(&mut self) {
        let Some(exporting) = draft_mut::<Exporting>(&mut self.sheet) else {
            return;
        };
        if exporting.is_asking() || self.job.is_some() {
            return;
        }
        let Some(order) = exporting.order(self.document.project()) else {
            return;
        };
        let Some(compute) = rikka_picture::SharedGpu::compute(Duration::ZERO) else {
            exporting.fail(tr!("cut.export.no_gpu"));
            return;
        };
        let imaging = Imaging::shared(&compute);
        let preview = ComputedImage::default();
        let preview_frame = preview.clone();
        let sender = self.sender.clone();
        let started = Job::start(order, imaging, move |report| match report {
            Report::Previewed(frame) => preview_frame.show(frame),
            report => {
                let _ = sender.send(move |cut: &mut Cut| cut.hear_export(report));
            }
        });
        match started {
            Ok(job) => {
                self.preferences.keep_export(exporting.settings());
                self.preferences.keep_export_folder(exporting.folder());
                self.export_preview = preview;
                self.sheet = None;
                self.job = Some(job);
            }
            Err(trouble) => {
                exporting.fail(tr!("cut.export.unstarted", reason = &trouble.to_string()))
            }
        }
    }

    pub(crate) fn hear_export(&mut self, report: Report) {
        let Some(job) = &mut self.job else {
            skip_view();
            return;
        };
        job.hear(report);
        if matches!(job.phase(), Phase::Cancelled) {
            self.drop_export();
        }
    }

    pub(crate) fn cancel_export(&mut self) {
        if let Some(job) = &mut self.job {
            job.cancel();
        }
    }

    pub(crate) fn close_export(&mut self) {
        if self.job.as_ref().is_some_and(Job::is_over) {
            self.drop_export();
        }
    }

    fn drop_export(&mut self) {
        self.job = None;
        self.export_preview = ComputedImage::default();
    }

    fn exported(&self) -> Option<&Path> {
        match self.job.as_ref()?.phase() {
            Phase::Done(summary) => Some(&summary.path),
            _ => None,
        }
    }

    pub(crate) fn show_export(&mut self) {
        let Some(path) = self.exported() else {
            return;
        };
        let revealed = rikka_open::reveal(path);
        self.settle_opened(revealed);
    }

    pub(crate) fn open_export_result(&mut self) {
        let Some(path) = self.exported() else {
            return;
        };
        let opened = rikka_open::open(&rikka_open::path_to_uri(path));
        self.settle_opened(opened);
    }

    fn settle_opened(&mut self, opened: rikka_open::Result<()>) {
        match opened {
            Ok(()) => self.close_export(),
            Err(trouble) => {
                self.notice = Some(tr!("cut.export.unopened", reason = &trouble.to_string()));
            }
        }
    }
}
