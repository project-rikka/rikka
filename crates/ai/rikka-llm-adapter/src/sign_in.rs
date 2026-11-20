use crate::login::{BrowserLogin, DeviceCodeLogin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignIn {
    Key { variable: &'static str },
    Browser,
    Code,
    KeyOrBrowser { variable: &'static str },
}

impl SignIn {
    #[must_use]
    pub fn variable(self) -> Option<&'static str> {
        match self {
            SignIn::Key { variable } | SignIn::KeyOrBrowser { variable } => Some(variable),
            SignIn::Browser | SignIn::Code => None,
        }
    }

    #[must_use]
    pub fn is_interactive(self) -> bool {
        matches!(
            self,
            SignIn::Browser | SignIn::Code | SignIn::KeyOrBrowser { .. }
        )
    }
}

pub enum SignInMethod<T: 'static> {
    Key {
        variable: &'static str,
    },
    Browser(fn() -> BrowserLogin<T>),
    Code(fn() -> DeviceCodeLogin<T>),
    KeyOrBrowser {
        variable: &'static str,
        browser: fn() -> BrowserLogin<T>,
    },
}

impl<T> SignInMethod<T> {
    #[must_use]
    pub const fn kind(&self) -> SignIn {
        match self {
            SignInMethod::Key { variable } => SignIn::Key { variable },
            SignInMethod::Browser(_) => SignIn::Browser,
            SignInMethod::Code(_) => SignIn::Code,
            SignInMethod::KeyOrBrowser { variable, .. } => SignIn::KeyOrBrowser { variable },
        }
    }
}

impl<T> std::fmt::Debug for SignInMethod<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind().fmt(formatter)
    }
}
