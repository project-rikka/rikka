use files_convert::{Quality, Request, Target, Tuning};
use files_core::Location;
use rikka_image::ColorScheme;
use rikka_ui::theme;

use crate::window::archives::local;
use crate::window::jobs::After;
use crate::window::{Converting, Dialog, WindowState};
use crate::wording;

pub(crate) const QUALITIES: [Quality; 3] = [Quality::Standard, Quality::High, Quality::Best];

impl WindowState {
    pub(super) fn conversions(&self) -> Vec<Target> {
        let tab = self.tabs.active();
        if !tab.at().is_local() || !tab.folder_allows().create || tab.any_folder_selected() {
            return Vec::new();
        }
        let entries = tab.selected_entries();
        files_convert::offers(
            entries
                .iter()
                .map(|entry| (entry.name.as_str(), &entry.mime)),
        )
    }

    pub(super) fn convert_to(&mut self, target: Target) {
        let sources = self.tabs.active().selection();
        self.start_conversion(&sources, target, Tuning::suited(target, shown_scheme()));
    }

    pub(super) fn show_convert(&mut self) {
        let mut targets = self.conversions();
        targets.retain(|target| target.extension().is_some());
        if targets.is_empty() {
            return;
        }
        self.dialog.show(Dialog::Convert(Converting {
            sources: self.tabs.active().selection(),
            targets,
            target: 0,
            quality: 0,
            size: 0,
        }));
    }

    pub(crate) fn convert_format(&mut self, index: usize) {
        if let Some(Dialog::Convert(sheet)) = self.dialog.showing_mut() {
            sheet.target = index.min(sheet.targets.len() - 1);
            sheet.size = 0;
        }
    }

    pub(crate) fn convert_quality(&mut self, index: usize) {
        if let Some(Dialog::Convert(sheet)) = self.dialog.showing_mut() {
            sheet.quality = index.min(QUALITIES.len() - 1);
        }
    }

    pub(crate) fn convert_size(&mut self, index: usize) {
        if let Some(Dialog::Convert(sheet)) = self.dialog.showing_mut() {
            let sizes = sheet.chosen().sizes().len();
            sheet.size = index.min(sizes.saturating_sub(1));
        }
    }

    pub(super) fn convert_chosen(&mut self) {
        let Some(Dialog::Convert(sheet)) = self.dialog.showing() else {
            return;
        };
        let target = sheet.chosen();
        let tuning = Tuning {
            quality: QUALITIES[sheet.quality],
            scale: target.sizes().get(sheet.size).copied().unwrap_or_default(),
            scheme: shown_scheme(),
        };
        let sources = sheet.sources.clone();
        self.dismiss();
        self.start_conversion(&sources, target, tuning);
    }

    fn start_conversion(&mut self, sources: &[Location], target: Target, tuning: Tuning) {
        let Some(sources) = local(sources) else {
            return;
        };
        let request = Request {
            sources,
            target,
            tuning,
            suffix: wording::conversion_suffix(target),
        };
        self.jobs
            .run(files_convert::convert(request), After::Nothing);
    }
}

fn shown_scheme() -> ColorScheme {
    match theme().is_dark() {
        true => ColorScheme::Dark,
        false => ColorScheme::Light,
    }
}
