use std::path::PathBuf;

use rikka_types::WindowIdentifier;

use crate::error::Result;
use crate::filter::Filter;
use crate::run;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChooserResponse {
    Files(Vec<PathBuf>),
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Purpose {
    #[default]
    Open,
    Save,
}

#[derive(Clone, Debug, Default)]
pub struct Chooser {
    pub(crate) purpose: Purpose,
    pub(crate) title: String,
    pub(crate) parent: Option<WindowIdentifier>,
    pub(crate) accept_label: String,
    pub(crate) folder: Option<PathBuf>,
    pub(crate) name: Option<String>,
    pub(crate) filters: Vec<Filter>,
    pub(crate) multiple: bool,
    pub(crate) directory: bool,
}

impl Chooser {
    #[must_use]
    pub fn open(title: impl Into<String>) -> Chooser {
        Chooser {
            title: title.into(),
            ..Chooser::default()
        }
    }

    #[must_use]
    pub fn save(title: impl Into<String>) -> Chooser {
        Chooser {
            purpose: Purpose::Save,
            title: title.into(),
            ..Chooser::default()
        }
    }

    #[must_use]
    pub fn parent(mut self, parent: Option<WindowIdentifier>) -> Chooser {
        self.parent = parent;
        self
    }

    #[must_use]
    pub fn accept_label(mut self, label: impl Into<String>) -> Chooser {
        self.accept_label = label.into();
        self
    }

    #[must_use]
    pub fn multiple(mut self, multiple: bool) -> Chooser {
        self.multiple = multiple;
        self
    }

    #[must_use]
    pub fn directory(mut self, directory: bool) -> Chooser {
        self.directory = directory;
        self
    }

    #[must_use]
    pub fn current_folder(mut self, folder: impl Into<PathBuf>) -> Chooser {
        self.folder = Some(folder.into());
        self
    }

    #[must_use]
    pub fn current_name(mut self, name: impl Into<String>) -> Chooser {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn filter(mut self, filter: Filter) -> Chooser {
        self.filters.push(filter);
        self
    }

    pub fn run(&self) -> Result<ChooserResponse> {
        run::run(self)
    }
}
