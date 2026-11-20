use rikka_gecko_protocol::geckoview::runtime::SetLocale;

#[must_use]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Locales {
    interface: Vec<String>,
    pages: Vec<String>,
}

impl Locales {
    pub fn new<S: Into<String>>(interface: impl IntoIterator<Item = S>) -> Locales {
        let interface: Vec<String> = interface.into_iter().map(Into::into).collect();
        Locales {
            pages: interface.clone(),
            interface,
        }
    }

    pub fn pages<S: Into<String>>(mut self, languages: impl IntoIterator<Item = S>) -> Locales {
        self.pages = languages.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.interface.is_empty() && self.pages.is_empty()
    }

    pub(crate) fn wire(&self) -> SetLocale {
        SetLocale {
            accept_languages: self.pages.join(", "),
            requested_locales: (!self.interface.is_empty()).then(|| self.interface.clone()),
        }
    }
}
