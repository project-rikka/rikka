use base64::Engine;
use base64::engine::general_purpose::STANDARD;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Media {
    Png,
    Jpeg,
    Gif,
    WebP,
    Pdf,
    PlainText,
}

impl Media {
    #[must_use]
    pub fn from_mime(mime: &str) -> Option<Media> {
        let named = mime.split(';').next().unwrap_or_default().trim();
        match named.to_ascii_lowercase().as_str() {
            "image/png" => Some(Media::Png),
            "image/jpeg" | "image/jpg" => Some(Media::Jpeg),
            "image/gif" => Some(Media::Gif),
            "image/webp" => Some(Media::WebP),
            "application/pdf" => Some(Media::Pdf),
            "text/plain" => Some(Media::PlainText),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_picture(self) -> bool {
        matches!(self, Media::Png | Media::Jpeg | Media::Gif | Media::WebP)
    }

    #[must_use]
    pub fn mime(self) -> &'static str {
        match self {
            Media::Png => "image/png",
            Media::Jpeg => "image/jpeg",
            Media::Gif => "image/gif",
            Media::WebP => "image/webp",
            Media::Pdf => "application/pdf",
            Media::PlainText => "text/plain",
        }
    }

    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Media::Png => "png",
            Media::Jpeg => "jpg",
            Media::Gif => "gif",
            Media::WebP => "webp",
            Media::Pdf => "pdf",
            Media::PlainText => "txt",
        }
    }

    #[must_use]
    pub fn data_url(self, data: &[u8]) -> String {
        format!("data:{};base64,{}", self.mime(), STANDARD.encode(data))
    }
}

impl std::fmt::Display for Media {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.mime())
    }
}
