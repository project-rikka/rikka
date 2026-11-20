use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    engine: String,
    terms: String,
    url: Url,
    keyword: bool,
}

impl Search {
    pub(crate) fn new(engine: &str, terms: &str, url: Url, keyword: bool) -> Search {
        Search {
            engine: engine.to_owned(),
            terms: terms.to_owned(),
            url,
            keyword,
        }
    }

    #[must_use]
    pub fn engine(&self) -> &str {
        &self.engine
    }

    #[must_use]
    pub fn terms(&self) -> &str {
        &self.terms
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn by_keyword(&self) -> bool {
        self.keyword
    }
}
