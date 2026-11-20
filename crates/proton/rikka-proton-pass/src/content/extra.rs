use crate::content::timestamp::Timestamp;

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ExtraTotp {
    #[prost(string, tag = "1")]
    pub totp_uri: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ExtraTextField {
    #[prost(string, tag = "1")]
    pub content: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ExtraHiddenField {
    #[prost(string, tag = "1")]
    pub content: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ExtraTimestampField {
    #[prost(message, optional, tag = "1")]
    pub timestamp: Option<Timestamp>,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ExtraField {
    #[prost(string, tag = "1")]
    pub field_name: String,
    #[prost(oneof = "ExtraFieldContent", tags = "2, 3, 4, 5")]
    pub content: Option<ExtraFieldContent>,
}

#[derive(Clone, PartialEq, Eq, prost::Oneof)]
pub enum ExtraFieldContent {
    #[prost(message, tag = "2")]
    Totp(ExtraTotp),
    #[prost(message, tag = "3")]
    Text(ExtraTextField),
    #[prost(message, tag = "4")]
    Hidden(ExtraHiddenField),
    #[prost(message, tag = "5")]
    Timestamp(ExtraTimestampField),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtraValue<'a> {
    Totp(&'a str),
    Text(&'a str),
    Hidden(&'a str),
    Timestamp(Timestamp),
}

impl ExtraField {
    pub fn name(&self) -> &str {
        &self.field_name
    }

    pub fn value(&self) -> Option<ExtraValue<'_>> {
        match self.content.as_ref()? {
            ExtraFieldContent::Totp(field) => Some(ExtraValue::Totp(&field.totp_uri)),
            ExtraFieldContent::Text(field) => Some(ExtraValue::Text(&field.content)),
            ExtraFieldContent::Hidden(field) => Some(ExtraValue::Hidden(&field.content)),
            ExtraFieldContent::Timestamp(field) => {
                Some(ExtraValue::Timestamp(field.timestamp.unwrap_or_default()))
            }
        }
    }

    pub fn totp_uri(&self) -> Option<&str> {
        match self.value()? {
            ExtraValue::Totp(uri) if !uri.is_empty() => Some(uri),
            _ => None,
        }
    }

    pub fn is_hidden(&self) -> bool {
        matches!(self.content, Some(ExtraFieldContent::Hidden(_)))
    }
}
