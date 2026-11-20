use rikka_sync::Mutex;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use http::{HeaderName, HeaderValue};
use url::Url;

use crate::Flow;
use crate::Overwrite;
use crate::auth::Auth;
use crate::cancel::CancelToken;
use crate::checksum::Checksum;
use crate::client::Client;
use crate::download::engine::Engine;
use crate::download::summary::DownloadSummary;
use crate::error::Result;
use crate::progress::{Progress, ProgressCallback};
use crate::request_options::RequestOptions;
use crate::retry::RetryPolicy;
use crate::transfer::{self, Transfer};

const PART_SUFFIX: &str = ".part";

const META_SUFFIX: &str = ".part.meta";

pub struct DownloadBuilder {
    client: Client,
    url: Result<Url>,
    destination: PathBuf,
    overwrite: Overwrite,
    resume: bool,
    checksum: Option<Checksum>,
    options: RequestOptions,
    progress: Option<ProgressCallback>,
    retry: Option<RetryPolicy>,
}

impl DownloadBuilder {
    pub(crate) fn new(client: Client, url: Result<Url>, destination: PathBuf) -> DownloadBuilder {
        DownloadBuilder {
            client,
            url,
            destination,
            overwrite: Overwrite::default(),
            resume: true,
            checksum: None,
            options: RequestOptions::default(),
            progress: None,
            retry: None,
        }
    }

    #[must_use]
    pub fn overwrite(mut self, overwrite: Overwrite) -> Self {
        self.overwrite = overwrite;
        self
    }

    #[must_use]
    pub fn resume(mut self, resume: bool) -> Self {
        self.resume = resume;
        self
    }

    #[must_use]
    pub fn checksum(mut self, checksum: Checksum) -> Self {
        self.checksum = Some(checksum);
        self
    }

    #[must_use]
    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        self.options.header(name, value);
        self
    }

    #[must_use]
    pub fn auth(mut self, auth: Auth) -> Self {
        self.options.auth(auth);
        self
    }

    #[must_use]
    pub fn bearer_auth(self, token: impl Into<String>) -> Self {
        self.auth(Auth::bearer(token))
    }

    #[must_use]
    pub fn on_progress(mut self, progress: impl FnMut(Progress) -> Flow + Send + 'static) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    #[must_use]
    pub fn cancel_token(mut self, token: CancelToken) -> Self {
        self.options.set_cancel(token);
        self
    }

    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    pub async fn start(self) -> Result<DownloadSummary> {
        let (engine, progress) = self.split()?;
        engine.run(progress).await
    }

    #[must_use]
    pub fn spawn(mut self) -> Transfer<DownloadSummary> {
        let shared = Arc::new(Mutex::new(Progress::default()));
        let cancel = self.options.cancel_or_default();

        self.progress = Some(transfer::share(&shared, self.progress.take()));

        Transfer::spawn(shared, cancel, async move { self.start().await })
    }

    fn split(mut self) -> Result<(Engine, Option<ProgressCallback>)> {
        self.options.check()?;
        let url = self.url?;

        let engine = Engine {
            part: sidecar(&self.destination, PART_SUFFIX),
            meta: sidecar(&self.destination, META_SUFFIX),
            client: self.client,
            url,
            destination: self.destination,
            overwrite: self.overwrite,
            resume: self.resume,
            checksum: self.checksum,
            options: self.options,
            retry: self.retry.unwrap_or_default(),
        };

        Ok((engine, self.progress))
    }
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

impl fmt::Debug for DownloadBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DownloadBuilder")
            .field("url", &self.url)
            .field("destination", &self.destination)
            .field("overwrite", &self.overwrite)
            .field("resume", &self.resume)
            .field("checksum", &self.checksum)
            .finish_non_exhaustive()
    }
}
