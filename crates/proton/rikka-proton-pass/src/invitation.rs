use crate::api::MemberRole;
use crate::identifier::ItemId;
use jiff::Timestamp;

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    email: String,
    role: MemberRole,
    item: Option<ItemId>,
    expires_at: Option<Timestamp>,
}

impl Invitation {
    pub fn new(email: impl Into<String>, role: MemberRole) -> Invitation {
        Invitation {
            email: email.into(),
            role,
            item: None,
            expires_at: None,
        }
    }

    pub fn for_item(mut self, item: ItemId) -> Invitation {
        self.item = Some(item);
        self
    }

    pub fn with_expiration(mut self, at: Timestamp) -> Invitation {
        self.expires_at = Some(at);
        self
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn role(&self) -> MemberRole {
        self.role
    }

    #[must_use]
    pub fn item(&self) -> Option<&ItemId> {
        self.item.as_ref()
    }

    #[must_use]
    pub fn expires_at(&self) -> Option<Timestamp> {
        self.expires_at
    }
}
