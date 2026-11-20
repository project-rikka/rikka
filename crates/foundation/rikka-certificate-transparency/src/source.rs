use rikka_net::Url;

use crate::error::{Error, Result};
use crate::key::LogListKey;

const CHROME_LIST: &str = "https://www.gstatic.com/ct/log_list/v3/log_list.json";
const CHROME_SIGNATURE: &str = "https://www.gstatic.com/ct/log_list/v3/log_list.sig";

#[derive(Debug, Clone)]
pub struct LogListSource {
    list: Url,
    signature: Url,
    key: LogListKey,
}

impl LogListSource {
    #[must_use]
    pub fn chrome() -> LogListSource {
        LogListSource::new(CHROME_LIST, CHROME_SIGNATURE, LogListKey::chrome())
            .expect("Chrome's log list URLs should be valid")
    }

    pub fn new(list: &str, signature: &str, key: LogListKey) -> Result<LogListSource> {
        Ok(LogListSource {
            list: parse_url(list)?,
            signature: parse_url(signature)?,
            key,
        })
    }

    #[must_use]
    pub fn list_url(&self) -> &str {
        self.list.as_str()
    }

    #[must_use]
    pub fn signature_url(&self) -> &str {
        self.signature.as_str()
    }

    #[must_use]
    pub fn key(&self) -> &LogListKey {
        &self.key
    }
}

fn parse_url(url: &str) -> Result<Url> {
    Url::parse(url).map_err(|error| Error::Url {
        url: url.to_owned(),
        reason: error.to_string(),
    })
}
