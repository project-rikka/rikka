use crate::cached;
use crate::client::{LABELS, MailClient};
use crate::error::Result;
use crate::label::{Label, LabelType};

#[must_use]
#[derive(Debug, Clone)]
pub struct LabelQuery {
    client: MailClient,
    kinds: Vec<LabelType>,
}

impl LabelQuery {
    pub(crate) fn new(client: MailClient) -> LabelQuery {
        LabelQuery {
            client,
            kinds: Vec::new(),
        }
    }

    pub fn kind(mut self, kind: LabelType) -> Self {
        if !self.kinds.contains(&kind) {
            self.kinds.push(kind);
        }
        self
    }

    pub fn kinds(mut self, kinds: impl IntoIterator<Item = LabelType>) -> Self {
        for kind in kinds {
            self = self.kind(kind);
        }
        self
    }

    pub async fn list(self) -> Result<Vec<Label>> {
        let mut labels = Vec::new();

        for kind in self.requested_types() {
            let response: Vec<Label> = self
                .client
                .api()
                .get(LABELS)
                .query(&[("Type", kind.number().to_string())])
                .send_field_or_default("Labels")
                .await?;

            if let Some(cache) = self.client.cache() {
                cached::cache_labels(cache, kind, &response);
            }
            labels.extend(response);
        }

        Ok(labels)
    }

    #[must_use]
    pub fn cached(&self) -> Option<Vec<Label>> {
        let cache = self.client.cache()?;
        let mut labels = Vec::new();
        for kind in self.requested_types() {
            labels.extend(cached::labels(cache, kind)?);
        }
        Some(labels)
    }

    fn requested_types(&self) -> Vec<LabelType> {
        match self.kinds.is_empty() {
            true => LabelType::all().to_vec(),
            false => self.kinds.clone(),
        }
    }
}
