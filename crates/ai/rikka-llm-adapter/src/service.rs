use crate::sign_in::{SignIn, SignInMethod};

pub struct Service<T: 'static> {
    pub id: &'static str,
    pub label: &'static str,
    pub sign_in: SignInMethod<T>,
}

impl<T> Service<T> {
    #[must_use]
    pub const fn sign_in(&self) -> SignIn {
        self.sign_in.kind()
    }
}

impl<T> std::fmt::Debug for Service<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Service")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("sign_in", &self.sign_in)
            .finish()
    }
}
