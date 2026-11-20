use rikka_proton_api::{Page, Paging};

use crate::client::{MESSAGES, MailClient};
use crate::error::Result;
use crate::identifier::MessageId;

const MAX_LIMIT: u32 = 1000;
const LIMIT: &str = "Limit";
const AFTER_ID: &str = "AfterID";

#[must_use]
#[derive(Debug, Clone)]
pub struct MessageIds {
    client: MailClient,
    after: Option<MessageId>,
    limit: u32,
}

impl MessageIds {
    pub(crate) fn new(client: MailClient) -> MessageIds {
        MessageIds {
            client,
            after: None,
            limit: MAX_LIMIT,
        }
    }

    pub fn starting_after(mut self, id: &MessageId) -> Self {
        self.after = Some(id.clone());
        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = limit.clamp(1, MAX_LIMIT);
        self
    }

    pub async fn list(&self) -> Result<Vec<MessageId>> {
        self.page(self.after.as_ref()).await
    }

    pub async fn all(&self) -> Result<Vec<MessageId>> {
        let mut paging = Paging::cursor(AFTER_ID);
        if let Some(after) = &self.after {
            paging = paging.starting_from(after);
        }
        self.client
            .api()
            .get(format!("{MESSAGES}/ids"))
            .query(&[(LIMIT, self.limit.to_string())])
            .pages(paging)
            .field("IDs")
            .collect(|ids: Vec<MessageId>| {
                let next = ids.last().cloned();
                Page::new(ids).next(next)
            })
            .await
            .map_err(Into::into)
    }

    async fn page(&self, after: Option<&MessageId>) -> Result<Vec<MessageId>> {
        let mut query: Vec<(&str, String)> = vec![(LIMIT, self.limit.to_string())];
        if let Some(after) = after {
            query.push((AFTER_ID, after.to_string()));
        }

        Ok(self
            .client
            .api()
            .get(format!("{MESSAGES}/ids"))
            .query(&query)
            .send_field_or_default("IDs")
            .await?)
    }
}
