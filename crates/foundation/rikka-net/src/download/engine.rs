use std::path::{Path, PathBuf};
use std::time::Instant;

use futures_util::StreamExt;
use http::{StatusCode, header};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use url::Url;

use crate::Overwrite;
use crate::cancel::{self, CancelGuard};
use crate::checksum::{Checksum, ChecksumAlgorithm};
use crate::client::Client;
use crate::download::content_range;
use crate::download::resume::ResumeState;
use crate::download::summary::DownloadSummary;
use crate::error::{Error, Result, exhausted};
use crate::hasher::Hasher;
use crate::progress::ProgressCallback;
use crate::reporter::Reporter;
use crate::request_options::RequestOptions;
use crate::response::Response;
use crate::retry::RetryPolicy;

const VERIFY_CHUNK: usize = 256 * 1024;

const WRITE_BUFFER: usize = 1024 * 1024;

pub(crate) struct Engine {
    pub(crate) client: Client,
    pub(crate) url: Url,
    pub(crate) destination: PathBuf,
    pub(crate) part: PathBuf,
    pub(crate) meta: PathBuf,
    pub(crate) overwrite: Overwrite,
    pub(crate) resume: bool,
    pub(crate) checksum: Option<Checksum>,
    pub(crate) options: RequestOptions,
    pub(crate) retry: RetryPolicy,
}

enum Attempt {
    Transferred { status: StatusCode, url: Url },
    Restart,
}

impl Engine {
    pub(crate) async fn run(self, callback: Option<ProgressCallback>) -> Result<DownloadSummary> {
        let started = Instant::now();

        if !self.may_overwrite() && tokio::fs::try_exists(&self.destination).await? {
            return self.finish_existing(started).await;
        }

        if let Some(parent) = self
            .destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        let (mut state, mut offset) = self.resume_point().await;
        if offset == 0 {
            self.discard_partial().await;
        }

        let resumed = offset > 0;
        let mut reporter = Reporter::new(
            state.as_ref().and_then(ResumeState::total),
            offset,
            callback,
            self.options.cancel().cloned(),
        );
        reporter.report()?;

        let mut attempt = 0u32;
        let mut restarted = false;

        let mut digest = (offset == 0)
            .then(|| self.checksum.as_ref().map(|c| Hasher::new(c.algorithm())))
            .flatten();

        let (status, url) = loop {
            match self
                .fetch(offset, state.as_ref(), &mut reporter, digest.as_mut())
                .await
            {
                Ok(Attempt::Transferred { status, url }) => break (status, url),
                Ok(Attempt::Restart) => {
                    digest = None;
                    if restarted {
                        return Err(Error::Protocol(
                            "server repeatedly rejected the download".to_owned(),
                        ));
                    }
                    tracing::debug!("server rejected range request, restarting download");
                    restarted = true;
                    self.discard_partial().await;
                    state = None;
                    offset = 0;
                    reporter.resumed_from(0);
                }
                Err(error) => {
                    if error.is_cancelled() {
                        return Err(error);
                    }
                    digest = None;

                    let delay = error
                        .is_transient()
                        .then(|| self.retry.delay(attempt, error.retry_after()))
                        .flatten();
                    let Some(delay) = delay else {
                        return Err(exhausted(attempt, error));
                    };

                    tracing::debug!(
                        %error,
                        attempt = attempt + 1,
                        ?delay,
                        "resuming download after failure",
                    );
                    cancel::sleep(delay, self.options.cancel()).await?;

                    attempt += 1;
                    (state, offset) = self.resume_point().await;
                    reporter.resumed_from(offset);
                }
            }
        };

        let bytes = file_len(&self.part).await;
        let checksum = match &self.checksum {
            Some(expected) => match self.verify(expected, digest).await {
                Ok(checksum) => Some(checksum),
                Err(error) => {
                    self.discard_partial().await;
                    return Err(error);
                }
            },
            None => None,
        };

        if !self.commit().await? {
            self.discard_partial().await;
            return self.finish_existing(started).await;
        }
        let _ = tokio::fs::remove_file(&self.meta).await;

        reporter.set_total(Some(bytes));
        reporter.finish();

        Ok(DownloadSummary {
            path: self.destination,
            url,
            status,
            bytes,
            resumed,
            skipped: false,
            elapsed: started.elapsed(),
            checksum,
        })
    }

    async fn resume_point(&self) -> (Option<ResumeState>, u64) {
        if !self.resume {
            return (None, 0);
        }

        let Some(state) = ResumeState::load(&self.meta)
            .await
            .filter(|state| state.is_for(&self.url))
        else {
            return (None, 0);
        };

        match file_len(&self.part).await {
            0 => (None, 0),
            offset => (Some(state), offset),
        }
    }

    async fn fetch(
        &self,
        offset: u64,
        state: Option<&ResumeState>,
        reporter: &mut Reporter,
        digest: Option<&mut Hasher>,
    ) -> Result<Attempt> {
        let mut request = self.options.apply(
            self.client
                .get(self.url.clone())
                .header(header::ACCEPT_ENCODING, "identity")
                .retry(RetryPolicy::never()),
        );

        if offset > 0 {
            request = request.header(header::RANGE, format!("bytes={offset}-"));
            if let Some(validator) = state.and_then(ResumeState::validator) {
                request = request.header(header::IF_RANGE, validator);
            }
        }

        let response = request.send().await?;

        if response.status() == StatusCode::RANGE_NOT_SATISFIABLE && offset > 0 {
            if state.and_then(ResumeState::total) == Some(offset) {
                return Ok(Attempt::Transferred {
                    status: response.status(),
                    url: response.url().clone(),
                });
            }
            return Ok(Attempt::Restart);
        }

        let response = response.error_for_status().await?;
        let status = response.status();
        let url = response.url().clone();

        let total = if status == StatusCode::PARTIAL_CONTENT {
            let range = response
                .headers()
                .get(header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(content_range::parse)
                .ok_or_else(|| {
                    Error::Protocol(
                        "a partial response arrived without a usable Content-Range".to_owned(),
                    )
                })?;

            if range.start != offset {
                return Err(Error::Protocol(format!(
                    "server resumed at byte {} instead of {offset}",
                    range.start
                )));
            }
            range.total
        } else {
            response.content_length()
        };

        let append = offset > 0 && status == StatusCode::PARTIAL_CONTENT;
        if !append && offset > 0 {
            reporter.resumed_from(0);
        }
        reporter.set_total(total);
        reporter.report()?;

        if self.resume {
            ResumeState::new(&self.url, response.headers(), total)
                .save(&self.meta)
                .await?;
        }

        self.write(response, append, reporter, digest).await?;
        Ok(Attempt::Transferred { status, url })
    }

    async fn write(
        &self,
        response: Response,
        append: bool,
        reporter: &mut Reporter,
        digest: Option<&mut Hasher>,
    ) -> Result<()> {
        let mut options = tokio::fs::OpenOptions::new();
        options.create(true).write(true);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }

        let mut writer = BufWriter::with_capacity(WRITE_BUFFER, options.open(&self.part).await?);
        let result = self.copy(response, &mut writer, reporter, digest).await;

        let flushed = writer.flush().await;
        let synced = writer.into_inner().sync_all().await;

        result?;
        flushed?;
        synced?;
        Ok(())
    }

    async fn copy(
        &self,
        response: Response,
        writer: &mut BufWriter<tokio::fs::File>,
        reporter: &mut Reporter,
        mut digest: Option<&mut Hasher>,
    ) -> Result<()> {
        let mut stream = Box::pin(response.bytes_stream());

        while let Some(chunk) =
            cancel::until_cancelled(stream.next(), self.options.cancel()).await?
        {
            let chunk = chunk?;
            writer.write_all(&chunk).await?;

            if let Some(digest) = digest.as_deref_mut() {
                digest.update(&chunk);
            }
            reporter.advance(chunk.len() as u64)?;
        }

        Ok(())
    }

    async fn verify(&self, expected: &Checksum, streamed: Option<Hasher>) -> Result<Checksum> {
        let digest = match streamed {
            Some(hasher) => hasher.finish(),
            None => self.digest_written_file(expected.algorithm()).await?,
        };

        expected.verify(&digest)?;
        Checksum::new(expected.algorithm(), digest)
    }

    async fn digest_written_file(&self, algorithm: ChecksumAlgorithm) -> Result<Vec<u8>> {
        let mut file = tokio::fs::File::open(&self.part).await?;
        let mut hasher = Hasher::new(algorithm);
        let mut buffer = vec![0u8; VERIFY_CHUNK];

        loop {
            if let Some(token) = self.options.cancel() {
                token.guard()?;
            }
            match file.read(&mut buffer).await? {
                0 => break,
                read => hasher.update(&buffer[..read]),
            }
        }

        Ok(hasher.finish())
    }

    fn may_overwrite(&self) -> bool {
        self.overwrite == Overwrite::Replace
    }

    async fn commit(&self) -> Result<bool> {
        if self.may_overwrite() {
            tokio::fs::rename(&self.part, &self.destination).await?;
            return Ok(true);
        }
        match rikka_fs::rename_new(&self.part, &self.destination) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    async fn finish_existing(self, started: Instant) -> Result<DownloadSummary> {
        if self.overwrite == Overwrite::Fail {
            return Err(Error::AlreadyExists(self.destination));
        }
        Ok(DownloadSummary {
            bytes: file_len(&self.destination).await,
            path: self.destination,
            url: self.url,
            status: StatusCode::OK,
            resumed: false,
            skipped: true,
            elapsed: started.elapsed(),
            checksum: None,
        })
    }

    async fn discard_partial(&self) {
        let _ = tokio::fs::remove_file(&self.part).await;
        let _ = tokio::fs::remove_file(&self.meta).await;
    }
}

async fn file_len(path: &Path) -> u64 {
    tokio::fs::metadata(path)
        .await
        .map_or(0, |metadata| metadata.len())
}
