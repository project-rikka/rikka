#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserVerification {
    Required,
    #[default]
    Preferred,
    Discouraged,
}

impl UserVerification {
    #[must_use]
    pub fn from_name(name: &str) -> Option<UserVerification> {
        match name {
            "required" => Some(UserVerification::Required),
            "preferred" => Some(UserVerification::Preferred),
            "discouraged" => Some(UserVerification::Discouraged),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            UserVerification::Required => "required",
            UserVerification::Preferred => "preferred",
            UserVerification::Discouraged => "discouraged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResidentKey {
    #[default]
    Discouraged,
    Preferred,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Algorithm(i64);

impl Algorithm {
    pub const ES256: Algorithm = Algorithm(-7);
    pub const EDDSA: Algorithm = Algorithm(-8);
    pub const ES384: Algorithm = Algorithm(-35);
    pub const RS256: Algorithm = Algorithm(-257);

    #[must_use]
    pub const fn new(id: i64) -> Algorithm {
        Algorithm(id)
    }

    #[must_use]
    pub const fn id(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelyingParty {
    id: String,
    name: Option<String>,
}

impl RelyingParty {
    #[must_use]
    pub fn new(id: impl Into<String>) -> RelyingParty {
        RelyingParty {
            id: id.into(),
            name: None,
        }
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> RelyingParty {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: Vec<u8>,
    name: Option<String>,
    display_name: Option<String>,
}

impl User {
    #[must_use]
    pub fn new(id: impl Into<Vec<u8>>) -> User {
        User {
            id: id.into(),
            name: None,
            display_name: None,
        }
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> User {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> User {
        self.display_name = Some(display_name.into());
        self
    }

    #[must_use]
    pub fn id(&self) -> &[u8] {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}
