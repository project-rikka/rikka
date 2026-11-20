use jiff::Timestamp;
use rikka_proton_api::Outcome;
use serde::{Deserialize, Serialize};

use crate::client::{CONVERSATIONS, MESSAGES, MailClient, PAGE};
use crate::error::Result;
use crate::identifier::{ConversationId, LabelId, MessageId};
use crate::outcome::{BatchOutcome, UndoToken};

mod sealed {
    pub trait Sealed {}
}

pub trait BatchTarget: sealed::Sealed + Clone + Serialize {
    #[doc(hidden)]
    const PATH: &'static str;
}

impl sealed::Sealed for MessageId {}
impl sealed::Sealed for ConversationId {}

impl BatchTarget for MessageId {
    const PATH: &'static str = MESSAGES;
}

impl BatchTarget for ConversationId {
    const PATH: &'static str = CONVERSATIONS;
}

#[must_use]
#[derive(Debug)]
pub struct Batch<'a, I> {
    client: &'a MailClient,
    ids: &'a [I],
}

impl<I> Clone for Batch<'_, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I> Copy for Batch<'_, I> {}

#[must_use]
#[derive(Debug, Clone, Copy)]
pub struct ConversationsInLabel<'a> {
    batch: Batch<'a, ConversationId>,
    label: &'a LabelId,
}

impl<'a, I: BatchTarget> Batch<'a, I> {
    pub(crate) fn new(client: &'a MailClient, ids: &'a [I]) -> Batch<'a, I> {
        Batch { client, ids }
    }

    pub async fn mark_read(self) -> Result<BatchOutcome> {
        self.put("read", |ids| IdsRequest { ids }).await
    }

    pub async fn label(self, label: &LabelId) -> Result<BatchOutcome> {
        self.put("label", |ids| LabelRequest { ids, label }).await
    }

    pub async fn unlabel(self, label: &LabelId) -> Result<BatchOutcome> {
        self.put("unlabel", |ids| LabelRequest { ids, label }).await
    }

    async fn put<B: Serialize>(
        &self,
        action: &str,
        request_body: impl Fn(&'a [I]) -> B,
    ) -> Result<BatchOutcome> {
        let path = format!("{}/{action}", I::PATH);
        let mut outcome = BatchOutcome::default();

        for chunk in self.ids.chunks(PAGE as usize) {
            let response: BatchResponse = self
                .client
                .api()
                .put(path.as_str())
                .json(&request_body(chunk))
                .send()
                .await?;

            outcome.merge(response.responses, response.undo);
        }

        Ok(outcome)
    }
}

impl Batch<'_, MessageId> {
    pub async fn mark_unread(self) -> Result<BatchOutcome> {
        self.put("unread", |ids| IdsRequest { ids }).await
    }

    pub async fn delete(self) -> Result<BatchOutcome> {
        self.put("delete", |ids| IdsRequest { ids }).await
    }

    pub async fn expire(self, at: Option<Timestamp>) -> Result<BatchOutcome> {
        self.put("expire", |ids| ExpireRequest {
            ids,
            expiration_time: at,
        })
        .await
    }
}

impl<'a> Batch<'a, ConversationId> {
    pub fn in_label(self, label: &'a LabelId) -> ConversationsInLabel<'a> {
        ConversationsInLabel { batch: self, label }
    }

    pub async fn snooze(self, until: Timestamp) -> Result<BatchOutcome> {
        self.put("snooze", |ids| SnoozeRequest { ids, until }).await
    }

    pub async fn unsnooze(self) -> Result<BatchOutcome> {
        self.put("unsnooze", |ids| IdsRequest { ids }).await
    }
}

impl ConversationsInLabel<'_> {
    pub async fn mark_unread(self) -> Result<BatchOutcome> {
        let label = self.label;
        self.batch
            .put("unread", |ids| LabelRequest { ids, label })
            .await
    }

    pub async fn delete(self) -> Result<BatchOutcome> {
        let label = self.label;
        self.batch
            .put("delete", |ids| LabelRequest { ids, label })
            .await
    }
}

#[derive(Debug, Serialize)]
struct IdsRequest<'a, I> {
    #[serde(rename = "IDs")]
    ids: &'a [I],
}

#[derive(Debug, Serialize)]
struct LabelRequest<'a, I> {
    #[serde(rename = "IDs")]
    ids: &'a [I],
    #[serde(rename = "LabelID")]
    label: &'a LabelId,
}

#[derive(Debug, Serialize)]
struct ExpireRequest<'a, I> {
    #[serde(rename = "IDs")]
    ids: &'a [I],
    #[serde(
        rename = "ExpirationTime",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    expiration_time: Option<Timestamp>,
}

#[derive(Debug, Serialize)]
struct SnoozeRequest<'a, I> {
    #[serde(rename = "IDs")]
    ids: &'a [I],
    #[serde(rename = "SnoozeTime", with = "rikka_proton_api::unix_seconds")]
    until: Timestamp,
}

#[derive(Debug, Deserialize)]
struct BatchResponse {
    #[serde(rename = "Responses", default)]
    responses: Vec<Outcome<String>>,
    #[serde(rename = "UndoToken", default)]
    undo: Option<UndoToken>,
}
