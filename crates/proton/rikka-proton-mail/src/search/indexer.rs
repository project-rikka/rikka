use std::ops::ControlFlow;
use std::time::Duration;

use futures_util::stream::{self, StreamExt};

use crate::client::{MailClient, PAGE};
use crate::error::Result;
use crate::event::Event;
use crate::identifier::MessageId;
use crate::message::{Message, MessageMetadata};
use crate::search::index::{Cursor, SearchIndex};
use crate::sort::Sort;
use crate::system_label::SystemLabel;

const BATCH_SIZE: u32 = 50;
const BATCH_INTERVAL: Duration = Duration::from_millis(500);
const CONCURRENT_FETCHES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStep {
    Indexed(usize),
    Done,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct Indexer {
    client: MailClient,
    batch: u32,
    pause: Duration,
}

impl Indexer {
    pub(crate) fn new(client: MailClient) -> Indexer {
        Indexer {
            client,
            batch: BATCH_SIZE,
            pause: BATCH_INTERVAL,
        }
    }

    pub fn batch_size(mut self, size: u32) -> Self {
        self.batch = size.clamp(1, PAGE);
        self
    }

    pub fn pause(mut self, pause: Duration) -> Self {
        self.pause = pause;
        self
    }

    pub async fn step(&self, index: &mut SearchIndex) -> Result<IndexStep> {
        if index.is_complete() {
            return Ok(IndexStep::Done);
        }

        let mut query = self
            .client
            .messages()
            .label(&SystemLabel::AllMail.id())
            .sort(Sort::Time)
            .descending(true)
            .page_size(self.batch);
        if let Some(cursor) = index.cursor() {
            query = query.end(cursor.time).end_id(&cursor.id);
        }
        let page = query.list().await?;

        let Some(oldest) = page.last() else {
            index.finish();
            return Ok(IndexStep::Done);
        };
        let cursor = Cursor {
            time: oldest.time(),
            id: oldest.id().clone(),
        };
        if index.cursor() == Some(&cursor) {
            index.finish();
            return Ok(IndexStep::Done);
        }

        let is_last_page = page.len() < self.batch as usize;
        let unindexed: Vec<MessageId> = page
            .iter()
            .map(MessageMetadata::id)
            .filter(|id| !index.has_content(id))
            .cloned()
            .collect();
        let indexed = self.fill(index, &unindexed).await?;

        index.advance(cursor);
        if is_last_page {
            index.finish();
        }

        Ok(IndexStep::Indexed(indexed))
    }

    pub async fn run(
        &self,
        index: &mut SearchIndex,
        mut after_batch: impl FnMut(&SearchIndex, IndexStep) -> ControlFlow<()>,
    ) -> Result<()> {
        loop {
            let step = self.step(index).await?;
            if after_batch(index, step).is_break() || step == IndexStep::Done {
                return Ok(());
            }
            tokio::time::sleep(self.pause).await;
        }
    }

    pub async fn fill(&self, index: &mut SearchIndex, ids: &[MessageId]) -> Result<usize> {
        let mut filled = 0;

        for chunk in ids.chunks(self.batch as usize) {
            let fetched = stream::iter(chunk.iter().cloned())
                .map(|id| async move { self.client.message(&id).await })
                .buffer_unordered(CONCURRENT_FETCHES)
                .collect::<Vec<_>>()
                .await;

            let mut failure = None;
            let mut messages = Vec::new();
            for outcome in fetched {
                match outcome {
                    Ok(message) => messages.push(message),
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }

            filled += self.index_messages(index, messages).await;
            if let Some(error) = failure {
                return Err(error);
            }
        }

        Ok(filled)
    }

    pub async fn catch_up(&self, index: &mut SearchIndex, event: &Event) -> Result<usize> {
        let needs_content = index.apply(event);
        self.fill(index, &needs_content).await
    }

    async fn index_messages(&self, index: &mut SearchIndex, messages: Vec<Message>) -> usize {
        let decrypted = stream::iter(messages)
            .map(|message| async move {
                let text = match self.client.decrypt_message(&message).await {
                    Ok(decrypted) => Some(decrypted.content().text().to_owned()),
                    Err(error) => {
                        tracing::warn!(
                            id = %message.id(),
                            %error,
                            "failed to decrypt message for search index"
                        );
                        None
                    }
                };
                (message, text)
            })
            .buffer_unordered(CONCURRENT_FETCHES)
            .collect::<Vec<_>>()
            .await;

        let count = decrypted.len();
        for (message, text) in decrypted {
            index.insert(message.metadata().clone(), text.as_deref());
        }
        count
    }
}
