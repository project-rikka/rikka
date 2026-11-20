use std::borrow::Cow;

use url::Url;

use crate::error::{Error, Result};
use crate::search::Search;
use crate::template;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionFormat {
    OpenSearch,
    Google,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchEngine {
    id: Cow<'static, str>,
    name: Cow<'static, str>,
    keyword: Cow<'static, str>,
    search: Cow<'static, str>,
    suggestions: Option<Cow<'static, str>>,
    format: SuggestionFormat,
    home: Cow<'static, str>,
    builtin: bool,
}

impl SearchEngine {
    pub(crate) const fn builtin(
        id: &'static str,
        name: &'static str,
        keyword: &'static str,
        search: &'static str,
        suggestions: &'static str,
        format: SuggestionFormat,
        home: &'static str,
    ) -> SearchEngine {
        SearchEngine {
            id: Cow::Borrowed(id),
            name: Cow::Borrowed(name),
            keyword: Cow::Borrowed(keyword),
            search: Cow::Borrowed(search),
            suggestions: Some(Cow::Borrowed(suggestions)),
            format,
            home: Cow::Borrowed(home),
            builtin: true,
        }
    }

    pub fn builder(id: impl Into<String>) -> SearchEngineBuilder {
        SearchEngineBuilder {
            id: id.into(),
            name: String::new(),
            keyword: String::new(),
            search: String::new(),
            suggestions: None,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn keyword(&self) -> &str {
        &self.keyword
    }

    #[must_use]
    pub fn search_template(&self) -> &str {
        &self.search
    }

    #[must_use]
    pub fn suggestion_template(&self) -> Option<&str> {
        self.suggestions.as_deref()
    }

    #[must_use]
    pub fn suggestion_format(&self) -> SuggestionFormat {
        self.format
    }

    #[must_use]
    pub fn home_host(&self) -> &str {
        &self.home
    }

    #[must_use]
    pub fn is_builtin(&self) -> bool {
        self.builtin
    }

    #[must_use]
    pub fn search_url(&self, terms: &str) -> Url {
        template::expand(&self.search, terms).expect("search templates are validated")
    }

    #[must_use]
    pub fn suggestion_url(&self, terms: &str) -> Option<Url> {
        let template = self.suggestions.as_deref()?;
        Some(template::expand(template, terms).expect("suggestion templates are validated"))
    }

    #[must_use]
    pub fn search(&self, terms: &str) -> Search {
        Search::new(&self.id, terms, self.search_url(terms), false)
    }

    pub(crate) fn keyword_search(&self, terms: &str) -> Search {
        Search::new(&self.id, terms, self.search_url(terms), true)
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct SearchEngineBuilder {
    id: String,
    name: String,
    keyword: String,
    search: String,
    suggestions: Option<String>,
}

impl SearchEngineBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn keyword(mut self, keyword: impl Into<String>) -> Self {
        self.keyword = keyword.into();
        self
    }

    pub fn search_url(mut self, template: impl Into<String>) -> Self {
        self.search = template.into();
        self
    }

    pub fn suggestion_url(mut self, template: impl Into<String>) -> Self {
        self.suggestions = Some(template.into());
        self
    }

    pub fn build(self) -> Result<SearchEngine> {
        let id = self.id.trim();
        if id.is_empty() {
            return Err(Error::MissingId);
        }
        let name = self.name.trim();
        if name.is_empty() {
            return Err(Error::MissingName);
        }
        let keyword = self.keyword.trim().to_lowercase();
        if keyword.is_empty() || keyword.chars().any(char::is_whitespace) {
            return Err(Error::InvalidKeyword(self.keyword));
        }
        let search = self.search.trim();
        if search.is_empty() {
            return Err(Error::MissingTemplate);
        }
        template::validate(search)?;
        let suggestions = match self.suggestions.as_deref().map(str::trim) {
            Some(template) if !template.is_empty() => {
                template::validate(template)?;
                Some(Cow::Owned(template.to_owned()))
            }
            _ => None,
        };
        let home = template::expand(search, "")
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_default();
        Ok(SearchEngine {
            id: Cow::Owned(id.to_owned()),
            name: Cow::Owned(name.to_owned()),
            keyword: Cow::Owned(keyword),
            search: Cow::Owned(search.to_owned()),
            suggestions,
            format: SuggestionFormat::OpenSearch,
            home: Cow::Owned(home),
            builtin: false,
        })
    }
}
