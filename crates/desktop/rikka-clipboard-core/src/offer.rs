use std::sync::Arc;

pub const TEXT: &str = "text/plain;charset=utf-8";
pub const PNG: &str = "image/png";
pub const URI_LIST: &str = "text/uri-list";

pub const SECRET_HINT: &str = "x-kde-passwordManagerHint";
pub const SECRET: &str = "secret";

pub const TEXT_TYPES: [&str; 5] = [TEXT, "text/plain", "UTF8_STRING", "STRING", "TEXT"];

#[derive(Clone, Default)]
pub struct Offer {
    entries: Vec<(String, Arc<[u8]>)>,
}

impl Offer {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn with(mut self, mime: impl Into<String>, data: impl Into<Arc<[u8]>>) -> Self {
        let mime = mime.into();
        let data = data.into();
        match self.entries.iter_mut().find(|(known, _)| known == &mime) {
            Some(entry) => entry.1 = data,
            None => self.entries.push((mime, data)),
        }
        self
    }

    pub fn text(text: impl AsRef<str>) -> Self {
        Self::new().with_text(text)
    }

    pub fn png(data: impl Into<Arc<[u8]>>) -> Self {
        Self::new().with(PNG, data)
    }

    pub fn uris<S: AsRef<str>>(uris: &[S]) -> Self {
        Self::new().with_uris(uris)
    }

    #[must_use]
    pub fn with_text(self, text: impl AsRef<str>) -> Self {
        let bytes: Arc<[u8]> = Arc::from(text.as_ref().as_bytes());
        TEXT_TYPES
            .into_iter()
            .fold(self, |offer, mime| offer.with(mime, bytes.clone()))
    }

    #[must_use]
    pub fn with_uris<S: AsRef<str>>(self, uris: &[S]) -> Self {
        let mut list = String::new();
        for uri in uris {
            list.push_str(uri.as_ref());
            list.push_str("\r\n");
        }
        self.with(URI_LIST, Arc::<[u8]>::from(list.as_bytes()))
    }

    #[must_use]
    pub fn secret(self) -> Self {
        self.with(SECRET_HINT, SECRET.as_bytes())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn mime_types(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(mime, _)| mime.as_str())
    }

    pub fn data(&self, mime: &str) -> Option<Arc<[u8]>> {
        self.entries
            .iter()
            .find(|(known, _)| known == mime)
            .map(|(_, data)| data.clone())
    }

    pub fn entries(&self) -> &[(String, Arc<[u8]>)] {
        &self.entries
    }
}

impl std::fmt::Debug for Offer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Offer")
            .field("mime_types", &self.mime_types().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

#[must_use]
pub fn text_mime(offered: &[String]) -> Option<&str> {
    TEXT_TYPES
        .into_iter()
        .find_map(|preferred| offered.iter().find(|known| *known == preferred))
        .or_else(|| offered.iter().find(|known| known.starts_with("text/")))
        .map(String::as_str)
}

#[must_use]
pub fn uri_list(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

pub fn marks_secret(offered: &[String], read: impl FnOnce(&str) -> Option<Vec<u8>>) -> bool {
    offered.iter().any(|mime| mime == SECRET_HINT)
        && read(SECRET_HINT)
            .and_then(|value| String::from_utf8(value).ok())
            .is_none_or(|value| value.trim() == SECRET)
}
