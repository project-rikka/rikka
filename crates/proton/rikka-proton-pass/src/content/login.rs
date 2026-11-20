use crate::content::non_empty;
use crate::content::passkey::Passkey;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum AutofillMode {
    Default = 0,
    Exact = 1,
    Never = 2,
    StartWith = 3,
    Pattern = 4,
    RegularExpression = 5,
    ExactPath = 6,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AutofillUrl {
    #[prost(string, tag = "1")]
    pub url: String,
    #[prost(enumeration = "AutofillMode", tag = "2")]
    pub mode: i32,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct ItemLogin {
    #[prost(string, tag = "1")]
    pub item_email: String,
    #[prost(string, tag = "2")]
    pub password: String,
    #[prost(string, repeated, tag = "3")]
    pub urls: Vec<String>,
    #[prost(string, tag = "4")]
    pub totp_uri: String,
    #[prost(message, repeated, tag = "5")]
    pub passkeys: Vec<Passkey>,
    #[prost(string, tag = "6")]
    pub item_username: String,
    #[prost(message, repeated, tag = "7")]
    pub autofill_urls: Vec<AutofillUrl>,
}

impl ItemLogin {
    pub fn email(&self) -> Option<&str> {
        non_empty(&self.item_email)
    }

    pub fn username(&self) -> Option<&str> {
        non_empty(&self.item_username)
    }

    pub fn identifier(&self) -> Option<&str> {
        self.username().or_else(|| self.email())
    }

    pub fn totp(&self) -> Option<&str> {
        non_empty(&self.totp_uri)
    }

    pub fn has_password(&self) -> bool {
        !self.password.is_empty()
    }
}
