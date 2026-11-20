#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AuthenticatorEntryContentTotp {
    #[prost(string, tag = "1")]
    pub uri: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AuthenticatorEntryContentSteam {
    #[prost(string, tag = "1")]
    pub secret: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AuthenticatorEntryContent {
    #[prost(oneof = "EntryContent", tags = "1, 2")]
    pub content: Option<EntryContent>,
}

#[derive(Clone, PartialEq, Eq, prost::Oneof)]
pub enum EntryContent {
    #[prost(message, tag = "1")]
    Totp(AuthenticatorEntryContentTotp),
    #[prost(message, tag = "2")]
    Steam(AuthenticatorEntryContentSteam),
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AuthenticatorEntryMetadata {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub note: String,
    #[prost(string, tag = "3")]
    pub id: String,
}

#[derive(Clone, PartialEq, Eq, prost::Message)]
pub struct AuthenticatorEntry {
    #[prost(message, optional, tag = "1")]
    pub metadata: Option<AuthenticatorEntryMetadata>,
    #[prost(message, optional, tag = "2")]
    pub content: Option<AuthenticatorEntryContent>,
}
