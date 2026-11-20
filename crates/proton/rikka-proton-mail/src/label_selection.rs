use rikka_proton_account::AddressId;
use serde::Serialize;

use crate::client::{MESSAGES, MailClient};
use crate::error::Result;
use crate::identifier::LabelId;
use crate::outcome::UndoToken;

const UNDO: &str = "mail/v4/undoactions";

#[must_use]
#[derive(Debug, Clone, Copy)]
pub struct LabelSelection<'a> {
    client: &'a MailClient,
    labels: &'a [LabelId],
}

impl<'a> LabelSelection<'a> {
    pub(crate) fn new(client: &'a MailClient, labels: &'a [LabelId]) -> LabelSelection<'a> {
        LabelSelection { client, labels }
    }

    pub async fn mark_read(self) -> Result<()> {
        self.post("read", &Context::new(self.labels)).await
    }

    pub async fn mark_unread(self) -> Result<()> {
        self.post("unread", &Context::new(self.labels)).await
    }

    pub async fn relabel(self, add: &[LabelId], remove: &[LabelId]) -> Result<()> {
        self.post(
            "label",
            &Relabel {
                context: Context::new(self.labels),
                add,
                remove,
            },
        )
        .await
    }

    pub async fn move_to(self, destination: &LabelId) -> Result<()> {
        self.post(
            "move",
            &Move {
                context: Context::new(self.labels),
                destination,
            },
        )
        .await
    }

    async fn post<B: Serialize>(&self, action: &str, body: &B) -> Result<()> {
        self.client
            .api()
            .post(format!("{MESSAGES}/batch/{action}"))
            .json(body)
            .send_unit()
            .await?;
        Ok(())
    }
}

impl MailClient {
    pub fn in_labels<'a>(&'a self, labels: &'a [LabelId]) -> LabelSelection<'a> {
        LabelSelection::new(self, labels)
    }

    pub async fn empty_label(&self, label: &LabelId, address: Option<&AddressId>) -> Result<()> {
        let mut query = vec![("LabelID", label.as_str())];
        if let Some(address) = address {
            query.push(("AddressID", address.as_str()));
        }

        self.api()
            .delete(format!("{MESSAGES}/empty"))
            .query(&query)
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn undo(&self, token: &UndoToken) -> Result<()> {
        self.api()
            .post(UNDO)
            .json(&UndoRequest {
                token: token.token(),
            })
            .send_unit()
            .await?;
        Ok(())
    }
}

#[derive(Serialize)]
struct SearchContext<'a> {
    #[serde(rename = "LabelIDs")]
    labels: &'a [LabelId],
}

#[derive(Serialize)]
struct Context<'a> {
    #[serde(rename = "SearchContext")]
    context: SearchContext<'a>,
}

impl<'a> Context<'a> {
    fn new(labels: &'a [LabelId]) -> Context<'a> {
        Context {
            context: SearchContext { labels },
        }
    }
}

#[derive(Serialize)]
struct Relabel<'a> {
    #[serde(flatten)]
    context: Context<'a>,
    #[serde(rename = "AddLabelIDs")]
    add: &'a [LabelId],
    #[serde(rename = "RemoveLabelIDs")]
    remove: &'a [LabelId],
}

#[derive(Serialize)]
struct Move<'a> {
    #[serde(flatten)]
    context: Context<'a>,
    #[serde(rename = "DestinationLabelID")]
    destination: &'a LabelId,
}

#[derive(Serialize)]
struct UndoRequest<'a> {
    #[serde(rename = "Token")]
    token: &'a str,
}
