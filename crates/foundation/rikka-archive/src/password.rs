use std::fmt;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Password(Vec<u8>);

impl Password {
    #[must_use]
    pub fn new(password: &str) -> Password {
        Password(password.as_bytes().to_vec())
    }

    #[must_use]
    pub fn from_bytes(password: &[u8]) -> Password {
        Password(password.to_vec())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn to_seven_z(&self) -> sevenz_rust2::Password {
        sevenz_rust2::Password::from_raw(&self.0)
    }
}

impl From<&str> for Password {
    fn from(password: &str) -> Password {
        Password::new(password)
    }
}

impl From<String> for Password {
    fn from(password: String) -> Password {
        Password(password.into_bytes())
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            f.write_str("Password(empty)")
        } else {
            f.write_str("Password(…)")
        }
    }
}
