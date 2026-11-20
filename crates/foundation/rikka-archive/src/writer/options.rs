use crate::level::Level;
use crate::password::Password;
use crate::progress::{CancelToken, Flow, Progress, ProgressCallback};

#[derive(Default)]
pub struct CreateOptions<'a> {
    pub(crate) level: Level,
    pub(crate) password: Option<Password>,
    pub(crate) encrypt_header: bool,
    pub(crate) threads: Option<usize>,
    pub(crate) follow_symlinks: bool,
    pub(crate) cancel: Option<CancelToken>,
    pub(crate) progress: Option<ProgressCallback<'a>>,
}

impl<'a> CreateOptions<'a> {
    #[must_use]
    pub fn new() -> CreateOptions<'a> {
        CreateOptions::default()
    }

    #[must_use]
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    #[must_use]
    pub fn password(mut self, password: impl Into<Password>) -> Self {
        self.password = Some(password.into());
        self
    }

    #[must_use]
    pub fn encrypt_header(mut self, encrypt: bool) -> Self {
        self.encrypt_header = encrypt;
        self
    }

    #[must_use]
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads.max(1));
        self
    }

    #[must_use]
    pub fn follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }

    #[must_use]
    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.cancel = Some(token);
        self
    }

    #[must_use]
    pub fn on_progress(mut self, progress: impl FnMut(Progress<'_>) -> Flow + 'a) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    pub(crate) fn thread_count(&self) -> u32 {
        self.threads
            .and_then(|threads| u32::try_from(threads).ok())
            .map_or_else(crate::threads::available, |threads| threads.max(1))
    }
}

impl std::fmt::Debug for CreateOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateOptions")
            .field("level", &self.level)
            .field("password", &self.password)
            .field("encrypt_header", &self.encrypt_header)
            .field("threads", &self.threads)
            .field("follow_symlinks", &self.follow_symlinks)
            .finish_non_exhaustive()
    }
}
