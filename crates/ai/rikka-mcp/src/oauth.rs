#[must_use]
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Oauth {
    client: Option<String>,
    secret: Option<String>,
    port: Option<u16>,
}

impl Oauth {
    pub fn new() -> Oauth {
        Oauth::default()
    }

    pub fn client(mut self, id: impl Into<String>) -> Self {
        self.client = Some(id.into()).filter(|id| !id.trim().is_empty());
        self
    }

    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into()).filter(|secret| !secret.is_empty());
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port).filter(|port| *port != 0);
        self
    }

    #[must_use]
    pub fn client_id(&self) -> Option<&str> {
        self.client.as_deref()
    }

    #[must_use]
    pub fn client_secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }

    #[must_use]
    pub fn callback_port(&self) -> Option<u16> {
        self.port
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.client.is_none() && self.secret.is_none() && self.port.is_none()
    }
}

impl std::fmt::Debug for Oauth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Oauth")
            .field("client", &self.client)
            .field("secret", &self.secret.as_ref().map(|_| "[hidden]"))
            .field("port", &self.port)
            .finish()
    }
}
