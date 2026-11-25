use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use futures_util::{TryStreamExt, stream};
use launcher_meta::Sha1;
use rikka_net::{CancelToken, Checksum, Client, Flow};
use sha1::Digest;

use crate::error::{Error, Result};
use crate::progress::{Progress, Stage};

const PARALLEL: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Verify {
    #[default]
    Presence,
    Contents,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Task {
    pub(crate) url: String,
    pub(crate) path: PathBuf,
    pub(crate) sha1: Option<Sha1>,
    pub(crate) size: Option<u64>,
}

pub(crate) type Report = Arc<dyn Fn(Progress) + Send + Sync>;

pub(crate) struct Fetcher {
    http: Client,
    cancel: CancelToken,
}

fn sha1_of(path: &Path) -> std::io::Result<String> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha1::Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(rikka_types::hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

fn present(task: &Task, verify: Verify) -> bool {
    let Ok(metadata) = std::fs::metadata(&task.path) else {
        return false;
    };
    if task.size.is_some_and(|size| size != metadata.len()) {
        return false;
    }
    match (verify, &task.sha1) {
        (Verify::Contents, Some(sha1)) => {
            sha1_of(&task.path).is_ok_and(|actual| actual == sha1.as_str())
        }
        _ => true,
    }
}

pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => value,
        Err(error) => std::panic::resume_unwind(error.into_panic()),
    }
}

impl Fetcher {
    pub(crate) fn new(http: Client, cancel: CancelToken) -> Fetcher {
        Fetcher { http, cancel }
    }

    pub(crate) fn checked(&self) -> Result<()> {
        match self.cancel.is_cancelled() {
            true => Err(Error::Cancelled),
            false => Ok(()),
        }
    }

    pub(crate) async fn bytes(&self, url: &str) -> Result<Vec<u8>> {
        self.checked()?;
        let download = |source: rikka_net::Error| match source.is_cancelled() {
            true => Error::Cancelled,
            false => Error::Download {
                url: url.to_owned(),
                source,
            },
        };
        let response = self
            .http
            .get(url)
            .cancel_token(self.cancel.clone())
            .send()
            .await
            .map_err(download)?;
        let bytes = response
            .error_for_status()
            .await
            .map_err(download)?
            .bytes()
            .await
            .map_err(download)?;
        Ok(bytes.to_vec())
    }

    pub(crate) async fn file(&self, task: &Task, counted: Arc<AtomicU64>) -> Result<()> {
        self.checked()?;
        let mut builder = self
            .http
            .download(&task.url, &task.path)
            .resume(false)
            .cancel_token(self.cancel.clone());
        if let Some(sha1) = &task.sha1 {
            let checksum = Checksum::sha1(sha1.as_str()).map_err(|source| Error::Download {
                url: task.url.clone(),
                source,
            })?;
            builder = builder.checksum(checksum);
        }
        let mut seen = 0u64;
        let builder = builder.on_progress(move |progress| {
            counted.fetch_add(progress.transferred.saturating_sub(seen), Ordering::Relaxed);
            seen = seen.max(progress.transferred);
            Flow::Continue
        });
        match builder.start().await {
            Ok(_) => Ok(()),
            Err(source) if source.is_cancelled() => Err(Error::Cancelled),
            Err(source) => Err(Error::Download {
                url: task.url.clone(),
                source,
            }),
        }
    }

    pub(crate) async fn cached(&self, url: &str, path: &Path, sha1: &Sha1) -> Result<Vec<u8>> {
        let task = Task {
            url: url.to_owned(),
            path: path.to_owned(),
            sha1: Some(sha1.clone()),
            size: None,
        };
        let checked = task.clone();
        if !blocking(move || present(&checked, Verify::Contents)).await {
            self.file(&task, Arc::default()).await?;
        }
        std::fs::read(path).map_err(Error::io(path))
    }

    pub(crate) async fn fresh_or_cached(&self, url: &str, path: &Path) -> Result<Vec<u8>> {
        let task = Task {
            url: url.to_owned(),
            path: path.to_owned(),
            sha1: None,
            size: None,
        };
        match self.file(&task, Arc::default()).await {
            Ok(()) => {}
            Err(Error::Download { url, source }) if path.is_file() => {
                tracing::warn!(%url, "using cached metadata: {source}");
            }
            Err(error) => return Err(error),
        }
        std::fs::read(path).map_err(Error::io(path))
    }

    pub(crate) async fn all(
        &self,
        stage: Stage,
        tasks: Vec<Task>,
        verify: Verify,
        report: &Report,
    ) -> Result<()> {
        let missing = blocking(move || {
            tasks
                .into_iter()
                .filter(|task| !present(task, verify))
                .collect::<Vec<_>>()
        })
        .await;
        let total_bytes: u64 = missing.iter().filter_map(|task| task.size).sum();
        let total_files = missing.len();
        let bytes = Arc::new(AtomicU64::new(0));
        let files = AtomicUsize::new(0);
        report(Progress {
            stage,
            bytes: 0,
            total_bytes,
            files: 0,
            total_files,
        });
        stream::iter(missing.iter().map(Ok))
            .try_for_each_concurrent(PARALLEL, |task| {
                let bytes = Arc::clone(&bytes);
                let files = &files;
                async move {
                    self.file(task, Arc::clone(&bytes)).await?;
                    let done = files.fetch_add(1, Ordering::Relaxed) + 1;
                    report(Progress {
                        stage,
                        bytes: bytes.load(Ordering::Relaxed).min(total_bytes),
                        total_bytes,
                        files: done,
                        total_files,
                    });
                    Ok::<(), Error>(())
                }
            })
            .await
    }
}
