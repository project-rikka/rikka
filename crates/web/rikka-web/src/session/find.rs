use rikka_gecko_protocol::geckoview::find::FindInPage;

#[must_use]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Find {
    query: Option<String>,
    backwards: bool,
    links_only: bool,
    match_case: bool,
    whole_word: bool,
    match_diacritics: bool,
}

impl Find {
    pub fn text(query: impl Into<String>) -> Find {
        Find {
            query: Some(query.into()),
            ..Find::default()
        }
    }

    pub fn next() -> Find {
        Find::default()
    }

    pub fn previous() -> Find {
        Find {
            backwards: true,
            ..Find::default()
        }
    }

    pub fn links_only(mut self) -> Find {
        self.links_only = true;
        self
    }

    pub fn match_case(mut self) -> Find {
        self.match_case = true;
        self
    }

    pub fn whole_word(mut self) -> Find {
        self.whole_word = true;
        self
    }

    pub fn match_diacritics(mut self) -> Find {
        self.match_diacritics = true;
        self
    }

    pub(crate) fn wire(self) -> FindInPage {
        FindInPage {
            search_string: self.query,
            backwards: self.backwards,
            links_only: self.links_only,
            match_case: self.match_case,
            whole_word: self.whole_word,
            match_diacritics: self.match_diacritics,
        }
    }
}
