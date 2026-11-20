use super::entities::User;
use super::extensions::PrfValues;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assertion {
    pub(crate) credential_id: Vec<u8>,
    pub(crate) client_data_json: String,
    pub(crate) authenticator_data: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    pub(crate) user: Option<User>,
    pub(crate) appid: bool,
    pub(crate) prf: Option<PrfValues>,
    pub(crate) hmac_secret: Option<PrfValues>,
    pub(crate) large_blob: Option<Vec<u8>>,
}

impl Assertion {
    #[must_use]
    pub fn credential_id(&self) -> &[u8] {
        &self.credential_id
    }

    #[must_use]
    pub fn client_data_json(&self) -> &str {
        &self.client_data_json
    }

    #[must_use]
    pub fn authenticator_data(&self) -> &[u8] {
        &self.authenticator_data
    }

    #[must_use]
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    #[must_use]
    pub fn user_handle(&self) -> Option<&[u8]> {
        self.user.as_ref().map(User::id)
    }

    #[must_use]
    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    #[must_use]
    pub fn used_appid(&self) -> bool {
        self.appid
    }

    #[must_use]
    pub fn prf(&self) -> Option<&PrfValues> {
        self.prf.as_ref()
    }

    #[must_use]
    pub fn hmac_secret(&self) -> Option<&PrfValues> {
        self.hmac_secret.as_ref()
    }

    #[must_use]
    pub fn large_blob(&self) -> Option<&[u8]> {
        self.large_blob.as_deref()
    }
}
