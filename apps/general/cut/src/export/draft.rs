use std::path::{Path, PathBuf};
use std::sync::Arc;

use cut_export::{Order, Range, Settings, Span, Target};
use cut_project::Project;
use rikka_app::prelude::Label;
#[derive(Debug)]
pub(crate) struct Exporting {
    settings: Settings,
    range: Range,
    details: bool,
    folder: PathBuf,
    stem: String,
    path: PathBuf,
    confirmed: Option<PathBuf>,
    asking: bool,
    trouble: Option<Label>,
    frame: i64,
    key: u64,
}

pub(crate) struct Opening {
    pub(crate) settings: Settings,
    pub(crate) range: Range,
    pub(crate) folder: PathBuf,
    pub(crate) stem: String,
    pub(crate) frame: i64,
    pub(crate) key: u64,
}

impl Exporting {
    pub(crate) fn new(opening: Opening) -> Exporting {
        let mut exporting = Exporting {
            settings: opening.settings,
            range: opening.range,
            details: false,
            path: PathBuf::new(),
            folder: opening.folder,
            stem: opening.stem,
            confirmed: None,
            asking: false,
            trouble: None,
            frame: opening.frame,
            key: opening.key,
        };
        exporting.place();
        exporting
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn range(&self) -> Range {
        self.range
    }

    pub(crate) fn details(&self) -> bool {
        self.details
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn folder(&self) -> &Path {
        &self.folder
    }

    pub(crate) fn frame(&self) -> i64 {
        self.frame
    }

    pub(crate) fn key(&self) -> u64 {
        self.key
    }

    pub(crate) fn is_asking(&self) -> bool {
        self.asking
    }

    pub(crate) fn ask(&mut self, asking: bool) {
        self.asking = asking;
    }

    pub(crate) fn trouble(&self) -> Option<&Label> {
        self.trouble.as_ref()
    }

    pub(crate) fn fail(&mut self, trouble: Label) {
        self.trouble = Some(trouble);
    }

    pub(crate) fn tune(&mut self, change: impl FnOnce(&mut Settings)) {
        self.trouble = None;
        let extension = self.settings.extension();
        change(&mut self.settings);
        if self.settings.extension() != extension {
            self.place();
        }
    }

    pub(crate) fn select_range(&mut self, range: Range) {
        self.range = range;
    }

    pub(crate) fn toggle_details(&mut self) {
        self.details = !self.details;
    }

    pub(crate) fn selected(&mut self, selected: PathBuf) {
        let extension = self.settings.extension();
        let typed = selected
            .extension()
            .is_some_and(|current| current.eq_ignore_ascii_case(extension));
        let named = match typed {
            true => selected.file_stem(),
            false => selected.file_name(),
        };
        if let Some(named) = named {
            self.stem = named.to_string_lossy().into_owned();
        }
        if let Some(folder) = selected.parent() {
            self.folder = folder.to_path_buf();
        }
        self.confirmed = Some(selected).filter(|selected| *selected == self.named_path());
        self.place();
    }

    pub(crate) fn span(&self, project: &Project) -> Option<Span> {
        Span::resolve(project, self.range)
    }

    pub(crate) fn is_ready(&self, project: &Project) -> bool {
        !self.asking && (self.settings.target == Target::Still || self.span(project).is_some())
    }

    pub(crate) fn order(&self, project: &Arc<Project>) -> Option<Order> {
        let path = self.path.clone();
        let project = Arc::clone(project);
        let order = match self.settings.target {
            Target::Video => {
                let span = self.span(&project)?;
                Order::video(project, span, self.settings.video, path)
            }
            Target::Audio => {
                let span = self.span(&project)?;
                Order::audio(project, span, self.settings.audio, path)
            }
            Target::Still => Order::still(project, self.frame, self.settings.still, path),
        };
        Some(match self.confirmed.as_ref() == Some(&self.path) {
            true => order.overwriting(),
            false => order,
        })
    }

    fn named(&self) -> String {
        format!("{}.{}", self.stem, self.settings.extension())
    }

    fn named_path(&self) -> PathBuf {
        self.folder.join(self.named())
    }

    fn place(&mut self) {
        let named_path = self.named_path();
        self.path = match self.confirmed.as_ref() == Some(&named_path) {
            true => named_path,
            false => self
                .folder
                .join(rikka_format::unique_name(&self.named(), |name| {
                    self.folder.join(name).exists()
                })),
        };
    }
}
