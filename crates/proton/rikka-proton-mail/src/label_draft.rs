use serde::Serialize;

use crate::client::{LABELS, MailClient};
use crate::error::{Error, Result};
use crate::identifier::LabelId;
use crate::label::{Label, LabelType};

#[must_use]
#[derive(Debug, Clone)]
pub struct LabelDraft {
    client: MailClient,
    name: String,
    color: Option<String>,
    kind: LabelType,
    parent: Option<LabelId>,
    notify: Option<bool>,
    display: Option<bool>,
}

impl LabelDraft {
    pub(crate) fn new(client: MailClient, name: impl Into<String>) -> LabelDraft {
        LabelDraft {
            client,
            name: name.into(),
            color: None,
            kind: LabelType::Label,
            parent: None,
            notify: None,
            display: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn kind(mut self, kind: LabelType) -> Self {
        self.kind = kind;
        self
    }

    pub fn parent(mut self, parent: &LabelId) -> Self {
        self.parent = Some(parent.clone());
        self
    }

    pub fn notify(mut self, notify: bool) -> Self {
        self.notify = Some(notify);
        self
    }

    pub fn display(mut self, display: bool) -> Self {
        self.display = Some(display);
        self
    }

    pub async fn create(self) -> Result<Label> {
        let request = LabelRequest {
            kind: Some(self.kind),
            ..self.to_request()?
        };
        Ok(self
            .client
            .api()
            .post(LABELS)
            .json(&request)
            .send_field("Label")
            .await?)
    }

    pub async fn update(self, id: &LabelId) -> Result<Label> {
        if id.is_system() {
            return Err(Error::SystemLabel(id.clone()));
        }

        let request = self.to_request()?;
        Ok(self
            .client
            .api()
            .put(format!("{LABELS}/{id}"))
            .json(&request)
            .send_field("Label")
            .await?)
    }

    fn to_request(&self) -> Result<LabelRequest> {
        if self.name.trim().is_empty() {
            return Err(Error::MissingLabelName);
        }

        Ok(LabelRequest {
            name: self.name.clone(),
            color: self.color.clone(),
            kind: None,
            parent: self.parent.clone(),
            notify: self.notify,
            display: self.display,
        })
    }
}

#[derive(Debug, Serialize)]
struct LabelRequest {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Color", skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(rename = "Type", skip_serializing_if = "Option::is_none")]
    kind: Option<LabelType>,
    #[serde(rename = "ParentID", skip_serializing_if = "Option::is_none")]
    parent: Option<LabelId>,
    #[serde(
        rename = "Notify",
        skip_serializing_if = "Option::is_none",
        serialize_with = "format_flag"
    )]
    notify: Option<bool>,
    #[serde(
        rename = "Display",
        skip_serializing_if = "Option::is_none",
        serialize_with = "format_flag"
    )]
    display: Option<bool>,
}

fn format_flag<S: serde::Serializer>(
    value: &Option<bool>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_u8(u8::from(*value)),
        None => serializer.serialize_none(),
    }
}
