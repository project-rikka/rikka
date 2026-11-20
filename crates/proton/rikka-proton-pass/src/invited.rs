#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invited {
    email: String,
    new_user: bool,
}

impl Invited {
    pub(crate) fn new(email: String, new_user: bool) -> Invited {
        Invited { email, new_user }
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn is_new_user(&self) -> bool {
        self.new_user
    }
}
