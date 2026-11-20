use crate::error::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trouble {
    reason: String,
    verification_url: Option<String>,
}

impl Trouble {
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Trouble {
        Trouble {
            reason: reason.into(),
            verification_url: None,
        }
    }

    #[must_use]
    pub fn caused_by(self, cause: &Error) -> Trouble {
        Trouble {
            verification_url: cause.verification_url().map(str::to_owned),
            ..self
        }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    #[must_use]
    pub fn verification_url(&self) -> Option<&str> {
        self.verification_url.as_deref()
    }
}

impl From<&Error> for Trouble {
    fn from(error: &Error) -> Trouble {
        Trouble::new(error.to_string()).caused_by(error)
    }
}
