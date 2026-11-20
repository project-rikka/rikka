use serde_json::Value;
use url::Url;

use crate::engine::SuggestionFormat;
use crate::error::{Error, Result};

const MAX_SUGGESTIONS: usize = 20;
const MAX_TEXT: usize = 512;
const XSSI_PREFIX: &[u8] = b")]}'";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSuggestion {
    pub text: String,
    pub url: Option<Url>,
    pub relevance: Option<u32>,
}

impl SearchSuggestion {
    #[must_use]
    pub fn query(text: impl Into<String>) -> SearchSuggestion {
        SearchSuggestion {
            text: text.into(),
            url: None,
            relevance: None,
        }
    }

    #[must_use]
    pub fn navigation(url: Url) -> SearchSuggestion {
        SearchSuggestion {
            text: url.to_string(),
            url: Some(url),
            relevance: None,
        }
    }
}

pub(crate) fn parse(format: SuggestionFormat, body: &[u8]) -> Result<Vec<SearchSuggestion>> {
    let body = body.strip_prefix(XSSI_PREFIX).unwrap_or(body);
    let value: Value = serde_json::from_slice(body).map_err(|_| Error::Malformed)?;
    let Value::Array(parts) = value else {
        return Err(Error::Malformed);
    };
    let Some(Value::Array(texts)) = parts.get(1) else {
        return Err(Error::Malformed);
    };
    let extras = match format {
        SuggestionFormat::Google => google_extras(&parts),
        SuggestionFormat::OpenSearch => None,
    };
    let mut suggestions = Vec::with_capacity(texts.len().min(MAX_SUGGESTIONS));
    for (index, text) in texts.iter().enumerate() {
        if suggestions.len() == MAX_SUGGESTIONS {
            break;
        }
        let Some(text) = text.as_str().map(str::trim) else {
            continue;
        };
        if text.is_empty() || text.len() > MAX_TEXT || text.chars().any(char::is_control) {
            continue;
        }
        let kind = extras.and_then(|extras| extras.get("google:suggesttype")?.get(index)?.as_str());
        let relevance = extras
            .and_then(|extras| extras.get("google:suggestrelevance")?.get(index)?.as_u64())
            .map(|relevance| u32::try_from(relevance).unwrap_or(u32::MAX));
        let url = match kind {
            Some("NAVIGATION") => match Url::parse(text) {
                Ok(url) if matches!(url.scheme(), "http" | "https") => Some(url),
                _ => continue,
            },
            _ => None,
        };
        suggestions.push(SearchSuggestion {
            text: text.to_owned(),
            url,
            relevance,
        });
    }
    Ok(suggestions)
}

fn google_extras(parts: &[Value]) -> Option<&serde_json::Map<String, Value>> {
    parts
        .iter()
        .skip(2)
        .filter_map(Value::as_object)
        .find(|object| object.contains_key("google:suggesttype"))
}
