use std::sync::Arc;

use rikka_tool::Media;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    source: DocumentSource,
    title: Option<String>,
    cached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentSource {
    Inline { media: Media, data: Arc<[u8]> },
    Url(String),
}

impl Document {
    #[must_use]
    pub fn pdf(data: impl Into<Arc<[u8]>>) -> Document {
        Document::inline(Media::Pdf, data)
    }

    #[must_use]
    pub fn text(text: impl Into<String>) -> Document {
        Document::inline(Media::PlainText, text.into().into_bytes())
    }

    #[must_use]
    pub fn inline(media: Media, data: impl Into<Arc<[u8]>>) -> Document {
        Document {
            source: DocumentSource::Inline {
                media,
                data: data.into(),
            },
            title: None,
            cached: false,
        }
    }

    #[must_use]
    pub fn url(url: impl Into<String>) -> Document {
        Document {
            source: DocumentSource::Url(url.into()),
            title: None,
            cached: false,
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn cached(mut self) -> Self {
        self.cached = true;
        self
    }

    #[must_use]
    pub fn source(&self) -> &DocumentSource {
        &self.source
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn is_cached(&self) -> bool {
        self.cached
    }
}
