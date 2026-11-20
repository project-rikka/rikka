use std::path::PathBuf;
use std::sync::Arc;

use animate_export::{Export, Job, Report, Reporter, Request};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};

use super::saving::with_extension;
use crate::animate::Animate;

impl Animate {
    pub(crate) fn can_export(&self) -> bool {
        !self.cast.figures().is_empty() && self.job.is_none()
    }

    pub(crate) fn job(&self) -> Option<&Job> {
        self.job.as_ref()
    }

    pub(crate) fn open_export(&mut self) {
        if self.transport.is_playing() {
            self.transport.pause();
        }
        self.output.start_exporting(
            self.transport.last_frame(),
            self.transport.sound().is_some(),
        );
    }

    pub(crate) fn export(&mut self) {
        let Some(exporting) = self.output.exporting_mut() else {
            return;
        };
        if exporting.is_asking() {
            return;
        }
        let Some(export) = exporting.export() else {
            return;
        };
        exporting.ask(true);
        let container = export.encoding.container;
        let chooser = Chooser::save(tr!("animate.export.choose").to_string())
            .parent(window_identifier())
            .accept_label(tr!("animate.export.accept").to_string())
            .current_name(format!("{}.{}", self.film_name(), container.extension()))
            .filter(
                Filter::named(animate_export::container_label(container).to_string())
                    .mime_type(container.mime_type()),
            );
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            move |animate: &mut Animate, chosen: rikka_dialog::Result<ChooserResponse>| {
                if let Some(exporting) = animate.output.exporting_mut() {
                    exporting.ask(false);
                }
                match chosen {
                    Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
                        Some(path) => {
                            animate
                                .begin_export(export, with_extension(path, container.extension()));
                        }
                        None => skip_view(),
                    },
                    Ok(ChooserResponse::Cancelled) => {}
                    Err(error) => animate.notice = Some(Label::from(error.to_string())),
                }
            },
        );
    }

    fn film_name(&self) -> String {
        self.cast.figures().first().map_or_else(
            || tr!("animate.title").to_string(),
            |figure| figure.name.clone(),
        )
    }

    fn begin_export(&mut self, export: Export, path: PathBuf) {
        if self.output.exporting().is_none() || self.job.is_some() {
            return;
        }
        self.folder = path.parent().map(PathBuf::from);
        self.output.settle(&export);
        let request = Request {
            stage: animate_stage::replica(&self.stage, &self.render),
            export,
            sound: self.transport.sound(),
            path,
        };
        let sender = self.sender.clone();
        let reporter: Reporter = Arc::new(move |report: Report| {
            let _ = sender.send(move |animate: &mut Animate| animate.on_export_report(report));
        });
        match Job::start(request, reporter) {
            Ok(job) => self.job = Some(job),
            Err(error) => self.notice = Some(Label::from(animate_ui::error_chain(&error))),
        }
    }

    pub(crate) fn on_export_report(&mut self, report: Report) {
        match report {
            Report::Cancelled => self.job = None,
            report => match &mut self.job {
                Some(job) => job.apply_report(report),
                None => skip_view(),
            },
        }
    }

    pub(crate) fn cancel_export(&mut self) {
        if let Some(job) = &mut self.job {
            job.cancel();
        }
    }

    pub(crate) fn close_export(&mut self) {
        if self.job.as_ref().is_some_and(Job::is_over) {
            self.job = None;
        }
    }

    pub(crate) fn play_export(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let opened = rikka_open::open(&rikka_open::path_to_uri(job.path()));
        self.settle_opened(opened);
    }

    pub(crate) fn show_export(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let revealed = rikka_open::reveal(job.path());
        self.settle_opened(revealed);
    }

    fn settle_opened(&mut self, opened: rikka_open::Result<()>) {
        match opened {
            Ok(()) => self.close_export(),
            Err(error) => self.notice = Some(Label::from(error.to_string())),
        }
    }
}
