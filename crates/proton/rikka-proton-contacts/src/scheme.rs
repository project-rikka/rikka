#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PgpScheme {
    Inline,
    Mime,
}

impl PgpScheme {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PgpScheme::Inline => "pgp-inline",
            PgpScheme::Mime => "pgp-mime",
        }
    }

    #[must_use]
    pub fn from_name(value: &str) -> Option<PgpScheme> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pgp-inline" => Some(PgpScheme::Inline),
            "pgp-mime" => Some(PgpScheme::Mime),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MimeType {
    Html,
    PlainText,
    Multipart,
}

impl MimeType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            MimeType::Html => "text/html",
            MimeType::PlainText => "text/plain",
            MimeType::Multipart => "multipart/mixed",
        }
    }

    #[must_use]
    pub fn from_vcard(value: &str) -> Option<MimeType> {
        value
            .trim()
            .eq_ignore_ascii_case(MimeType::PlainText.as_str())
            .then_some(MimeType::PlainText)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageScheme {
    Internal,
    EncryptedOutside,
    Clear,
    PgpInline,
    PgpMime,
}

impl PackageScheme {
    #[must_use]
    pub fn bits(self) -> i32 {
        match self {
            PackageScheme::Internal => 1,
            PackageScheme::EncryptedOutside => 2,
            PackageScheme::Clear => 4,
            PackageScheme::PgpInline => 8,
            PackageScheme::PgpMime => 16,
        }
    }
}
