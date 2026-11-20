use futures_util::{StreamExt, TryStreamExt, stream};
use music_core::{Error, Result};
use music_stream::{Fetcher, Plan, Spool, answer_bytes, answer_length, answer_text};

use crate::api::AGENT;

const AHEAD: usize = 4;
const MEASURED_AT_ONCE: usize = 16;
const TRIES: u32 = 3;

pub(crate) struct Cdn {
    fetcher: Fetcher,
}

impl Cdn {
    pub(crate) fn new() -> Cdn {
        Cdn {
            fetcher: Fetcher::new(AGENT).ahead(AHEAD).tries(TRIES),
        }
    }

    pub(crate) async fn text(&self, url: &str) -> Result<String> {
        answer_text(self.fetcher.client().get(url)).await
    }

    pub(crate) async fn bytes(&self, url: &str) -> Result<Vec<u8>> {
        Ok(answer_bytes(self.fetcher.client().get(url)).await?.to_vec())
    }

    pub(crate) async fn file(&self, url: &str) -> Result<Plan> {
        Ok(Plan::file(url, Some(self.length(url).await?)))
    }

    pub(crate) async fn segments(&self, urls: &[String]) -> Result<Plan> {
        let measured: Vec<(String, u64)> = stream::iter(urls.iter().cloned())
            .map(|url| async move {
                let length = self.length(&url).await?;
                Ok::<_, Error>((url, length))
            })
            .buffered(MEASURED_AT_ONCE)
            .try_collect()
            .await?;
        Ok(measured
            .into_iter()
            .fold(Plan::new(), |plan, (url, length)| {
                plan.piece(url, Some(length))
            }))
    }

    pub(crate) async fn download(&self, plan: Plan, spool: &mut Spool) -> Result<()> {
        self.fetcher.fetch(plan, spool).await
    }

    async fn length(&self, url: &str) -> Result<u64> {
        answer_length(self.fetcher.client().head(url)).await
    }
}
