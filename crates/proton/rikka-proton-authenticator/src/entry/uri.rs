use rikka_proton_api::percent;

use crate::error::{Error, Result};

pub(crate) struct Uri {
    scheme: String,
    body: String,
    query: Vec<(String, String)>,
}

impl Uri {
    pub(crate) fn parse(text: &str) -> Result<Uri> {
        let (scheme, rest) = text.split_once("://").ok_or(Error::MalformedUri)?;
        if scheme.is_empty() {
            return Err(Error::MalformedUri);
        }
        let (body, query) = rest.split_once('?').unwrap_or((rest, ""));
        Ok(Uri {
            scheme: scheme.to_ascii_lowercase(),
            body: body.to_owned(),
            query: query_pairs(query),
        })
    }

    pub(crate) fn scheme(&self) -> &str {
        &self.scheme
    }

    pub(crate) fn authority(&self) -> &str {
        self.body.split('/').next().unwrap_or_default()
    }

    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    pub(crate) fn path(&self) -> Option<&str> {
        let path = self.body.split_once('/')?.1;
        (!path.is_empty()).then_some(path)
    }

    pub(crate) fn query_param(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }
}

pub(crate) fn decode_component(text: &str) -> String {
    percent::decode(text).unwrap_or_else(|_| text.to_owned())
}

fn decode_query_component(text: &str) -> String {
    decode_component(&text.replace('+', " "))
}

fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode_query_component(key), decode_query_component(value))
        })
        .collect()
}
