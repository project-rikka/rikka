pub use rikka_types::Overwrite;

use crate::entry::Entry;
use crate::extract::report::MetadataReport;
use crate::password::Password;
use crate::progress::{CancelToken, Flow, Progress, ProgressCallback};

type EntryFilter<'a> = Box<dyn FnMut(&Entry) -> bool + 'a>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExtractSummary {
    pub files: usize,
    pub directories: usize,
    pub links: usize,
    pub skipped: usize,
    pub bytes: u64,
    pub metadata: MetadataReport,
}

#[derive(Default)]
pub struct ExtractOptions<'a> {
    pub(crate) overwrite: Overwrite,
    pub(crate) strip_components: usize,
    pub(crate) permissions: bool,
    pub(crate) timestamps: bool,
    pub(crate) symlinks: bool,
    pub(crate) password: Option<Password>,
    pub(crate) cancel: Option<CancelToken>,
    pub(crate) filter: Option<EntryFilter<'a>>,
    pub(crate) progress: Option<ProgressCallback<'a>>,
}

impl<'a> ExtractOptions<'a> {
    #[must_use]
    pub fn new() -> ExtractOptions<'a> {
        ExtractOptions {
            overwrite: Overwrite::default(),
            strip_components: 0,
            permissions: true,
            timestamps: true,
            symlinks: true,
            password: None,
            cancel: None,
            filter: None,
            progress: None,
        }
    }

    #[must_use]
    pub fn overwrite(mut self, overwrite: Overwrite) -> Self {
        self.overwrite = overwrite;
        self
    }

    #[must_use]
    pub fn strip_components(mut self, count: usize) -> Self {
        self.strip_components = count;
        self
    }

    #[must_use]
    pub fn permissions(mut self, restore: bool) -> Self {
        self.permissions = restore;
        self
    }

    #[must_use]
    pub fn timestamps(mut self, restore: bool) -> Self {
        self.timestamps = restore;
        self
    }

    #[must_use]
    pub fn symlinks(mut self, create: bool) -> Self {
        self.symlinks = create;
        self
    }

    #[must_use]
    pub fn password(mut self, password: impl Into<Password>) -> Self {
        self.password = Some(password.into());
        self
    }

    #[must_use]
    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.cancel = Some(token);
        self
    }

    #[must_use]
    pub fn filter(mut self, filter: impl FnMut(&Entry) -> bool + 'a) -> Self {
        self.filter = Some(Box::new(filter));
        self
    }

    #[must_use]
    pub fn on_progress(mut self, progress: impl FnMut(Progress<'_>) -> Flow + 'a) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }
}

impl std::fmt::Debug for ExtractOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtractOptions")
            .field("overwrite", &self.overwrite)
            .field("strip_components", &self.strip_components)
            .field("permissions", &self.permissions)
            .field("timestamps", &self.timestamps)
            .field("symlinks", &self.symlinks)
            .field("password", &self.password)
            .finish_non_exhaustive()
    }
}
