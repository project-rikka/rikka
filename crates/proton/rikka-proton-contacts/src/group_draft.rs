use serde::Serialize;

use crate::client::{ContactsClient, LABELS};
use crate::error::{Error, Result};
use crate::group::{CONTACT_GROUP, ContactGroup};
use crate::identifier::GroupId;

const DEFAULT_COLOR: &str = "#8080FF";

#[must_use]
#[derive(Debug, Clone)]
pub struct GroupDraft {
    client: ContactsClient,
    name: String,
    color: Option<String>,
}

#[derive(Serialize)]
struct GroupBody<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Color")]
    color: &'a str,
    #[serde(rename = "Type")]
    kind: i32,
}

impl GroupDraft {
    pub(crate) fn new(client: ContactsClient, name: impl Into<String>) -> GroupDraft {
        GroupDraft {
            client,
            name: name.into(),
            color: None,
        }
    }

    pub fn color(mut self, color: impl Into<String>) -> GroupDraft {
        self.color = Some(color.into());
        self
    }

    pub async fn create(self) -> Result<ContactGroup> {
        let body = self.body()?;
        Ok(self
            .client
            .api()
            .post(LABELS)
            .json(&body)
            .send_field("Label")
            .await?)
    }

    pub async fn update(self, id: &GroupId) -> Result<ContactGroup> {
        let body = self.body()?;
        Ok(self
            .client
            .api()
            .put(format!("{LABELS}/{id}"))
            .json(&body)
            .send_field("Label")
            .await?)
    }

    fn body(&self) -> Result<GroupBody<'_>> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(Error::MissingGroupName);
        }
        Ok(GroupBody {
            name,
            color: self.color.as_deref().unwrap_or(DEFAULT_COLOR),
            kind: CONTACT_GROUP,
        })
    }
}
