use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use futures_util::future::try_join_all;
use rikka_languages::Language;
use tokio::sync::Semaphore;

use crate::adapter::Adapter;
use crate::batch::batches;
use crate::cache::{Cache, CacheKey};
use crate::call::{Call, Translated};
use crate::error::{Error, Result};
use crate::http::Http;
use crate::job::Job;
use crate::markup::{Encoded, encode};
use crate::translation::Translation;

const REQUESTS_IN_FLIGHT: usize = 6;
const CACHE_BUDGET: usize = 8 << 20;

#[derive(Clone)]
pub struct Translator {
    inner: Arc<Inner>,
}

struct Inner {
    http: Http,
    cache: Mutex<Cache>,
    permits: Semaphore,
}

impl fmt::Debug for Translator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Translator").finish_non_exhaustive()
    }
}

impl Translator {
    pub fn new() -> Result<Translator> {
        Ok(Translator {
            inner: Arc::new(Inner {
                http: Http::new()?,
                cache: Mutex::new(Cache::new(CACHE_BUDGET)),
                permits: Semaphore::new(REQUESTS_IN_FLIGHT),
            }),
        })
    }

    pub async fn translate(&self, adapter: &dyn Adapter, job: Job) -> Result<Translation> {
        let service = adapter.service();
        if !service.serves(&job.to) {
            return Err(Error::Unsupported {
                service: service.label,
                language: job.to.tag(),
            });
        }
        let to = service.code(&job.to);
        let encoded: Vec<Option<Encoded>> = job
            .units
            .iter()
            .map(|unit| {
                unit.has_text()
                    .then(|| encode(unit, &service.dialect, job.protect))
            })
            .collect();

        let mut unique: Vec<&str> = Vec::new();
        let mut positions: HashMap<&str, usize> = HashMap::new();
        let slots: Vec<Option<usize>> = encoded
            .iter()
            .map(|encoded| {
                encoded.as_ref().map(|encoded| {
                    *positions.entry(encoded.html.as_str()).or_insert_with(|| {
                        unique.push(&encoded.html);
                        unique.len() - 1
                    })
                })
            })
            .collect();

        let key = |html: &str| CacheKey {
            service: service.id,
            to,
            protect: job.protect,
            html: html.to_owned(),
        };
        let mut results: Vec<Option<Arc<Translated>>> = {
            let mut cache = self.cache();
            unique.iter().map(|html| cache.get(&key(html))).collect()
        };
        let missing: Vec<(usize, &str)> = results
            .iter()
            .enumerate()
            .filter(|(_, result)| result.is_none())
            .map(|(index, _)| (index, unique[index]))
            .collect();
        let groups = batches(&missing, service.limits);
        let fetched = try_join_all(groups.iter().map(|group| {
            let strings: Vec<&str> = group.iter().map(|index| unique[*index]).collect();
            async move {
                let translated = self.fetch(adapter, &strings, to).await?;
                Ok::<_, Error>((group, translated))
            }
        }))
        .await?;
        {
            let mut cache = self.cache();
            for (group, translated) in fetched {
                for (index, translated) in group.iter().zip(translated) {
                    let translated = Arc::new(translated);
                    cache.insert(key(unique[*index]), Arc::clone(&translated));
                    results[*index] = Some(translated);
                }
            }
        }

        let mut outputs = Vec::with_capacity(job.units.len());
        let mut votes: Vec<(Language, usize)> = Vec::new();
        for ((unit, encoded), slot) in job.units.iter().zip(&encoded).zip(&slots) {
            let (Some(encoded), Some(slot)) = (encoded, slot) else {
                outputs.push(unit.untouched());
                continue;
            };
            let translated = results[*slot]
                .as_ref()
                .expect("every string is translated or cached");
            outputs.push(encoded.decode(&translated.text, &service.dialect));
            if let Some(language) = translated
                .detected
                .as_deref()
                .and_then(|code| service.language(code))
            {
                match votes.iter_mut().find(|(known, _)| *known == language) {
                    Some((_, count)) => *count += 1,
                    None => votes.push((language, 1)),
                }
            }
        }
        let detected = votes
            .iter()
            .enumerate()
            .max_by_key(|(order, (_, count))| (*count, std::cmp::Reverse(*order)))
            .map(|(_, (language, _))| *language);
        Ok(Translation { outputs, detected })
    }

    fn cache(&self) -> MutexGuard<'_, Cache> {
        self.inner.cache.lock()
    }

    async fn fetch(
        &self,
        adapter: &dyn Adapter,
        strings: &[&str],
        to: &'static str,
    ) -> Result<Vec<Translated>> {
        let _permit = self
            .inner
            .permits
            .acquire()
            .await
            .expect("request semaphore is never closed");
        let translated = adapter
            .translate(&self.inner.http, Call { strings, to })
            .await?;
        if translated.len() != strings.len() {
            return Err(Error::MalformedResponse);
        }
        Ok(translated)
    }
}
